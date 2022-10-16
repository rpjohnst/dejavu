use std::{env, io};
use std::error::Error;
use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    match build() {
        Ok(()) => { ExitCode::SUCCESS }
        Err(err) => {
            let _ = writeln!(io::stderr(), "{err}");
            ExitCode::FAILURE
        }
    }
}

fn build() -> Result<(), Box<dyn Error>> {
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();

    #[cfg(windows)]
    if target_family == "windows" {
        use std::{ffi::CStr, path::Path};
        use d3dcompiler::Shader;

        let main = CStr::from_bytes_with_nul(b"main\0").unwrap();
        let vs_5_0 = CStr::from_bytes_with_nul(b"vs_5_0\0").unwrap();
        let ps_5_0 = CStr::from_bytes_with_nul(b"ps_5_0\0").unwrap();
        d3dcompiler::compile(&[
            Shader {
                source: Path::new("src/graphics/vertex.hlsl"), target: Path::new("vertex.cso"),
                entry: main, model: vs_5_0,
            },
            Shader {
                source: Path::new("src/graphics/pixel.hlsl"), target: Path::new("pixel.cso"),
                entry: main, model: ps_5_0,
            },
        ])?;
    }
    #[cfg(not(windows))]
    if target_family == "windows" {
        use std::fmt;

        struct TargetError;

        impl fmt::Debug for TargetError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self)
            }
        }

        impl fmt::Display for TargetError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "d3dcompiler is not available for cross compilation")
            }
        }

        impl Error for TargetError {}

        return Err(Box::new(TargetError));
    }

    Ok(())
}
