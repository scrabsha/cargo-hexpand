use rustc_driver_impl::{Callbacks, Compilation, RunCompiler};
use rustc_interface::{interface, Queries};
use rustc_session::EarlyErrorHandler;

macro_rules! gen_compiler {
    (
        $(
            $step:ident:
                $(for< $( $lt:lifetime),* $(,)? >)?
                ( $( $in_name:ident: $in_ty:ty ),* $(,) ?)
                $( -> $out:ty )? ),* $(,)?
    ) => {
        pub(crate) struct Compiler<Payload> {
            $(
                $step: Option<Box<dyn $(for < $( $lt ,)* > )? FnOnce(&mut Payload, $( $in_ty ),* ) $( -> $out )? + Send + Sync>>,
            )*
            payload: Option<Payload>,
        }

        #[allow(dead_code)]
        impl<Payload> Compiler<Payload> where Payload: Send + Sync {
            pub(crate) fn new() -> Compiler<Payload>
            where
                Payload: Default
            {
                Compiler {
                    $(
                        $step: None,
                    )*
                    payload: Some(Payload::default())
                }
            }

            pub(crate) fn run(mut self, args: &[String]) -> Payload {
                RunCompiler::new(args, &mut self).run().unwrap();

                self.payload.take().unwrap()
            }

            $(
                pub(crate) fn $step<F>(mut self, f: F) -> Compiler<Payload>
                where F: $( for< $( $lt, )* >)? FnOnce( &mut Payload, $( $in_ty, )* ) $( -> $out )? + 'static + Send + Sync
                {
                    self.$step = Some(Box::new(f));
                    self
                }
            )*
        }

        impl<Payload> Callbacks for Compiler<Payload> {
            $(
                fn $step< $( $( $lt, )* )?>(&mut self, $( $in_name: $in_ty ),* ) $( -> $out )? {
                    if let Some(f) = self.$step.take() {
                        f( &mut self.payload.as_mut().unwrap(), $( $in_name ),* )
                    } $( else {
                        <$out>::Continue
                    })?
                }
            )*
        }

        impl<Payload> Drop for Compiler<Payload> {
            fn drop(&mut self) {
                if self.payload.is_some() {
                    panic!("Compiler not run");
                }
            }
        }
    };
}

gen_compiler! {
    config: (config: &mut interface::Config),
    after_parsing: (compiler: &interface::Compiler, queries: &Queries) -> Compilation,
    after_expansion: for<'tcx>(compiler: &interface::Compiler, queries: &'tcx Queries<'tcx>) -> Compilation,
    after_analysis: (handler: &EarlyErrorHandler, compiler: &interface::Compiler, queries: &Queries) -> Compilation,
}
