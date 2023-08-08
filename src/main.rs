#![feature(rustc_private)]

mod cli;
mod compiler;
mod expander;
mod pretty;

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

use rustc_driver_impl::Compilation;
use rustc_interface::interface;
use rustc_session::Session;
use rustc_span::FileName;

fn main() {
    compiler::Compiler::<()>::new()
        .after_expansion(|_, compiler_: &interface::Compiler, queries| {
            let (input, filename) = get_source(compiler_.session());
            expander::run(queries, filename, input);
            Compilation::Stop
        })
        .run(&cli::get_rustc_args());
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
