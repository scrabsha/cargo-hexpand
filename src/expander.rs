use std::collections::VecDeque;

use rustc_ast::visit::{self, Visitor};
use rustc_ast_pretty::pprust;
use rustc_interface::Queries;
use rustc_span::FileName;

use crate::pretty;

pub(crate) fn run<'tcx>(queries: &'tcx Queries<'tcx>, filename: FileName, input: String) {
    queries.global_ctxt().unwrap().enter(|tcx| {
        let krate = tcx.resolver_for_lowering(()).borrow().1.clone();

        // TODO: the following comment is unparseable for non-Sasha people.

        // Lang item capture step
        //
        // Long story short: we want to avoid emitting lang items as much
        // as possible because they may not be valid Rust cade (see:
        // `format_args`). In order to fix this, we visit the crate code
        // after (non lang-item) the macro expansion, before the HIR
        // lowering in order to capture actual compiling code for each lang
        // item.

        // TODO: audit all the lang items and see how we can capture each of
        // them and print them correctly.
        #[derive(Default)]
        struct MacroDiscoverer {
            format_args: VecDeque<String>,
        }

        impl<'ast> Visitor<'ast> for MacroDiscoverer {
            fn visit_expr(&mut self, ex: &'ast rustc_ast::Expr) {
                if matches!(&ex.kind, rustc_ast::ExprKind::FormatArgs(_)) {
                    let code = pprust::expr_to_string(ex);
                    self.format_args.push_back(code);
                }

                visit::walk_expr(self, ex);
            }
        }

        let mut discoverer = MacroDiscoverer::default();

        discoverer.visit_crate(&krate);

        let ann = pretty::Printer::new(&tcx, discoverer.format_args);

        let krate = tcx.hir().root_module();

        let attrs = [];

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
}
