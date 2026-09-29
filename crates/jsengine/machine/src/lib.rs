#![deny(unsafe_code)]

mod encoding;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[allow(unsafe_code)]
mod memory;
mod program;
pub use program::{Operation, Program};

pub struct Code {
    arity: usize,
    bytes: usize,
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    memory: memory::Memory,
}

impl Code {
    pub fn compile(program: &Program) -> Result<Self, String> {
        let words = encoding::emit(program);
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Ok(Self {
                arity: program.arity(),
                bytes: words.len() * 4,
                memory: memory::Memory::new(&words)?,
            })
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = words;
            Err("native backend requires macOS ARM64".into())
        }
    }

    pub fn run(&self, arguments: &[f64]) -> Result<f64, String> {
        if arguments.len() < self.arity {
            return Err("native argument buffer is too short".into());
        }
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Ok(self.memory.run(arguments))
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        Err("native backend requires macOS ARM64".into())
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }
}
