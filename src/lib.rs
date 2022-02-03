use std::error::Error;
use std::{panic::PanicInfo, sync::Arc};

/// The type of closures accepted in `evac`. Errors are printed to `stderr`.
pub type PanicHandler<T: 'static> =
    Box<dyn Fn(&PanicInfo<'_>, &mut T) -> Result<(), Box<dyn Error>> + 'static + Send + Sync>;

/// Builder for assembling a series of panic handlers. Because Rust only allows for a single panic
/// hook, this builder enables composing multiple panic handlers into a single hook.
///
/// ## Example
/// ```
/// # use std::fs::OpenOptions;
/// # use std::io::Write;
/// # use std::panic::PanicInfo;
/// # use std::path::PathBuf;
/// # fn get_dump(_: &PanicInfo) -> Vec<u8> { vec![] }
/// # let dump_path = PathBuf::from("")?;
/// EvacBuilder::new()
///   .with_handler(|panic_info, path| {
///     // Build the dump file
///     let mut dump: Vec<u8> = get_dump(panic_info);
///     // Write the dump to disk
///     let mut file = OpenOptions::new().write(true).open(path)?;
///     file.write_all(&mut dump)?;
///   })
///   .register(dump_path); // Register Evac with the path as the context
/// ```
pub struct EvacBuilder<T: 'static> {
    handlers: Vec<PanicHandler<T>>,
    preserve_default: bool,
}

impl<T: 'static> EvacBuilder<T> {
    /// Constructs a new [`EvalBuilder`]. Empty of handlers and does not preserve the default panic
    /// hook.
    pub fn new() -> Self {
        Self {
            handlers: vec![],
            preserve_default: false,
        }
    }

    /// Turns back on the default panic hook that ships with Rust.
    pub fn preserve_default_panic(mut self) -> Self {
        self.preserve_default = true;

        self
    }

    /// Adds a panic handler. Handlers are executed in the order they are registered in. They take
    /// a mutable reference to the context value so handlers can add to the context as they execute,
    /// enabling efficient reuse of values.
    pub fn with_handler(mut self, handler: PanicHandler<T>) -> Self {
        self.handlers.push(handler);

        self
    }

    /// Assembles and registers the supplied panic handlers as a serial panic handler.
    pub fn register(self, ctx: T) {
        let Self {
            handlers,
            preserve_default,
        } = self;

        // If we're preserving the default, pop it for use in our hook
        let default_hook = match preserve_default {
            true => Some(std::panic::take_hook()),
            false => None,
        };

        // Make the context Send + Sync; it'll be immutable until a panic occurs anyways
        let mut ctx = Arc::new(ctx);

        // Register our hook
        std::panic::set_hook(Box::new(move |info: &PanicInfo| {
            // Turn our Arc Context into a mutable reference.
            let ctx = Arc::get_mut(&mut ctx).unwrap();

            // If we popped the default hook, run it now
            if let Some(hook) = default_hook {
                hook(info);
            }

            // Run each registered handler, logging errors to stderr
            for hook in handlers {
                if let Err(e) = hook(info, ctx) {
                    eprintln!("Error encountered in panic handler:");
                    eprintln!("{e}");
                }
            }
        }));
    }
}
