use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{hash_map::Entry, HashMap, VecDeque},
};

use rustc_hir::{def::Res, intravisit::Map, Expr, ExprKind, HirId, Path, QPath};
use rustc_hir_pretty::{AnnNode, Nested, PpAnn, State};
use rustc_middle::ty::TyCtxt;
use rustc_span::symbol::Ident;

pub(crate) struct Printer<'a, 'hir> {
    tcx: Option<&'a TyCtxt<'hir>>,
    current_locals: RefCell<AnnotationState>,
    fmts: RefCell<VecDeque<String>>,
}

impl<'a, 'hir> Printer<'a, 'hir> {
    pub(crate) fn new(tcx: &'a TyCtxt<'hir>, fmts: VecDeque<String>) -> Printer<'a, 'hir> {
        let tcx = Some(tcx);
        let current_locals = RefCell::new(AnnotationState::new());
        let fmts = RefCell::new(fmts);

        Printer {
            tcx,
            current_locals,
            fmts,
        }
    }
}

impl<'a, 'hir> PpAnn for Printer<'a, 'hir> {
    fn nested(&self, state: &mut State<'_>, nested: Nested) {
        if let Some(tcx) = self.tcx {
            let should_be_cleader = matches!(nested, Nested::Body(_));

            PpAnn::nested(&(&tcx.hir() as &dyn Map<'_>), state, nested);

            if should_be_cleader {
                self.current_locals.borrow_mut().clear();
            }
        }
    }

    fn pre(&self, state: &mut State<'_>, node: AnnNode<'_>) {
        if let AnnNode::Expr(expr) = node {
            if patterns::is_format_args_call(expr) {
                // Emit a comment start. We will close it in the `post` method
                // and replace it with a the pre-MIR version.
                state.s.word(Cow::Borrowed("/* "))
            }
        }
    }

    fn post(&self, state: &mut State<'_>, node: rustc_hir_pretty::AnnNode<'_>) {
        match node {
            AnnNode::Expr(expr) => {
                if patterns::is_format_args_call(expr) {
                    // Finish the comment started at the `pre` step.
                    state.s.word(Cow::Borrowed(" */"));

                    // Ok so that's a lang item. Let's emit the original code
                    // instead. It may contain a macro call, but at least we're
                    // sure it compiles
                    let code = self.fmts.borrow_mut().pop_front().unwrap();
                    state.s.word(Cow::Owned(code));
                }

                if let Some(res_id) = expr_path_res_id(expr) {
                    if let Some(suffix) = self.current_locals.borrow_mut().suffix_of(res_id) {
                        let suffix = format!("_{suffix}");
                        state.s.word(Cow::Owned(suffix));
                    }
                }
            }

            AnnNode::Pat(pat) => {
                if let Some((def_id, ident)) = pat_ident_def_id(pat) {
                    if let Some(suffix) = self.current_locals.borrow_mut().new_ident(ident, def_id)
                    {
                        let suffix = format!("_{suffix}");
                        state.s.word(Cow::Owned(suffix));
                    }
                }
            }

            _ => {}
        }
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

// TODO: Move to a `pattern` module
mod patterns {
    use rustc_hir::{Expr, ExprKind, LangItem, QPath, Ty, TyKind};

    // TODO: Move to a `pattern` module
    pub(crate) fn is_format_args_call<'a>(e: &'a Expr) -> bool {
        matches!(
            e.kind,
            ExprKind::Call(
                Expr {
                    kind: ExprKind::Path(QPath::TypeRelative(
                        Ty {
                            kind: TyKind::Path(QPath::LangItem(LangItem::FormatArguments, _, _)),
                            ..
                        },
                        _,
                    )),
                    ..
                },
                _,
            )
        )
    }
}

// TODO: separate module?
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
