#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_ast_pretty;
extern crate rustc_attr;
extern crate rustc_driver;
extern crate rustc_driver_impl;
extern crate rustc_hir;
extern crate rustc_hir_pretty;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
};

use rustc_ast::{
    token::TokenKind,
    tokenstream::{DelimSpan, TokenStream},
    AttrArgs, Attribute, DelimArgs, MacDelimiter,
};
use rustc_attr::mk_attr;
use rustc_driver::Callbacks;
use rustc_driver_impl::{Compilation, RunCompiler};

use rustc_hir::{def::Res, intravisit::Map, Expr, ExprKind, HirId, Path, QPath, Ty, TyKind};
use rustc_hir_pretty::{AnnNode, Nested, PpAnn, State};
use rustc_middle::ty::TyCtxt;
use rustc_session::Session;
use rustc_span::{
    def_id::DefId, def_id::CRATE_DEF_ID, symbol::Ident, FileName, Symbol, SyntaxContext, DUMMY_SP,
};

fn main() {
    RunCompiler::new(
        &[
            "".to_string(),
            "--edition=2021".to_string(),
            "test.rs".to_string(),
        ],
        &mut Compiler,
    )
    .run()
    .unwrap();
}

struct Compiler;

impl Callbacks for Compiler {
    fn after_expansion<'tcx>(
        &mut self,
        compiler: &rustc_interface::interface::Compiler,
        queries: &'tcx rustc_interface::Queries<'tcx>,
    ) -> rustc_driver_impl::Compilation {
        let (input, filename) = get_source(compiler.session());

        queries.global_ctxt().unwrap().enter(|tcx| {
            let ann = ActuallyCorrectAnn::new(tcx);

            let krate = tcx.hir().root_module();

            let attrs = [
                feature(&tcx, "print_internals"),
                feature(&tcx, "fmt_internals"),
            ];

            let content = rustc_hir_pretty::print_crate(
                tcx.sess.source_map(),
                krate,
                filename,
                input,
                &|_| &attrs,
                &ann,
            );

            println!("{}", content);
        });

        Compilation::Stop
    }
}

fn feature(tcx: &TyCtxt, content: &str) -> Attribute {
    let feature_symbol = Symbol::intern("feature");
    let feature_ident = Ident::new(feature_symbol, DUMMY_SP);
    let feature_path = rustc_ast::Path::from_ident(feature_ident);

    let args = AttrArgs::Delimited(DelimArgs {
        dspan: DelimSpan::dummy(),
        delim: MacDelimiter::Parenthesis,
        tokens: TokenStream::token_alone(
            TokenKind::Ident(Symbol::intern(content), false),
            DUMMY_SP,
        ),
    });

    mk_attr(
        &tcx.sess.parse_sess.attr_id_generator,
        rustc_ast::AttrStyle::Inner,
        feature_path,
        args,
        DUMMY_SP
            .with_parent(Some(CRATE_DEF_ID))
            .with_ctxt(SyntaxContext::root()),
    )
}

struct ActuallyCorrectAnn<'hir> {
    tcx: Option<TyCtxt<'hir>>,
    current_locals: RefCell<AnnotationState>,
}

impl<'hir> ActuallyCorrectAnn<'hir> {
    fn new(tcx: TyCtxt<'hir>) -> ActuallyCorrectAnn<'hir> {
        let tcx = Some(tcx);

        ActuallyCorrectAnn {
            tcx,
            current_locals: RefCell::new(AnnotationState::new()),
        }
    }

    fn new_ident(&self, ident: Ident, id: HirId) -> Option<usize> {
        self.current_locals.borrow_mut().new_ident(ident, id)
    }

    fn suffix_of(&self, id: HirId) -> Option<usize> {
        self.current_locals.borrow().suffix_of(id)
    }
}

fn get_source(sess: &Session) -> (String, FileName) {
    let src_name = sess.io.input.source_name();
    let src = String::clone(
        sess.source_map()
            .get_source_file(&src_name)
            .expect("get_source_file")
            .src
            .as_ref()
            .expect("src"),
    );
    (src, src_name)
}

impl<'hir> PpAnn for ActuallyCorrectAnn<'hir> {
    fn nested(&self, state: &mut State<'_>, nested: Nested) {
        if let Some(tcx) = self.tcx {
            let should_be_cleader = matches!(nested, Nested::Body(_));

            PpAnn::nested(&(&tcx.hir() as &dyn Map<'_>), state, nested);

            if should_be_cleader {
                self.current_locals.borrow_mut().clear();
            }
        }
    }

    fn pre(&self, _state: &mut State<'_>, node: AnnNode<'_>) {
        // Put the lang item into comment
        if let AnnNode::Expr(expr) = node {
            if possible_lang_item(self.tcx.as_ref().unwrap(), expr).is_some() {
                _state.s.word(Cow::Borrowed("/*"));
            }
        }
    }

    fn post(&self, state: &mut State<'_>, node: rustc_hir_pretty::AnnNode<'_>) {
        match node {
            AnnNode::Expr(expr) => {
                if let Some((hir_id, method)) = possible_lang_item(self.tcx.as_ref().unwrap(), expr)
                {
                    state.s.word(Cow::Borrowed("*/"));
                    let resolved_lang_item = self.tcx.as_ref().unwrap().def_path_str(hir_id);
                    state.s.word(Cow::Owned(resolved_lang_item));
                    state.s.word(Cow::Borrowed("::"));
                    let method = method.as_str().to_string();
                    state.s.word(Cow::Owned(method));
                }

                if let Some(res_id) = expr_path_res_id(expr) {
                    if let Some(suffix) = self.suffix_of(res_id) {
                        let suffix = format!("_{suffix}");
                        state.s.word(Cow::Owned(suffix));
                    }
                }
            }

            AnnNode::Pat(pat) => {
                if let Some((def_id, ident)) = pat_ident_def_id(pat) {
                    if let Some(suffix) = self.new_ident(ident, def_id) {
                        let suffix = format!("_{suffix}");
                        state.s.word(Cow::Owned(suffix));
                    }
                }
            }

            _ => {}
        }
    }
}

fn expr_path_res_id(e: &Expr) -> Option<HirId> {
    if let ExprKind::Path(QPath::Resolved(
        _,
        Path {
            res: Res::Local(id),
            ..
        },
    )) = &e.kind
    {
        Some(*id)
    } else {
        None
    }
}

fn pat_ident_def_id(pat: &rustc_hir::Pat) -> Option<(HirId, Ident)> {
    if let rustc_hir::PatKind::Binding(_, id, ident, ..) = &pat.kind {
        Some((*id, ident.clone()))
    } else {
        None
    }
}

fn possible_lang_item<'a>(tcx: &TyCtxt, e: &'a Expr) -> Option<(DefId, &'a Ident)> {
    if let ExprKind::Path(QPath::TypeRelative(
        Ty {
            kind: TyKind::Path(QPath::LangItem(lang_item, _, _)),
            ..
        },
        segment,
    )) = &e.kind
    {
        tcx.lang_items()
            .get(*lang_item)
            .map(|id| (id, &segment.ident))
    } else {
        None
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AnnotationState {
    // Entry vacant means no suffix
    next_suffix: HashMap<Ident, usize>,
    current_suffixes: HashMap<HirId, Option<usize>>,
}

impl AnnotationState {
    fn new() -> AnnotationState {
        AnnotationState::default()
    }

    fn clear(&mut self) {
        self.next_suffix.clear();
        self.current_suffixes.clear();
    }

    fn new_ident(&mut self, ident: Ident, id: HirId) -> Option<usize> {
        let entry = self.next_suffix.entry(ident);
        let suffix = match entry {
            Entry::Occupied(mut entry) => {
                let suffix = *entry.get();
                *entry.get_mut() += 1;
                Some(suffix)
            }

            Entry::Vacant(entry) => {
                entry.insert(0);
                None
            }
        };

        self.current_suffixes.insert(id, suffix.clone());

        suffix
    }

    fn suffix_of(&self, id: HirId) -> Option<usize> {
        self.current_suffixes.get(&id).unwrap().as_ref().copied()
    }
}
