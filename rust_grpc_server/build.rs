use tonic_build;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("../protos/bidi_hello.proto")?;
    Ok(())
}
