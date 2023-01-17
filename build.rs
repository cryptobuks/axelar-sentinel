use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(&[
            "proto/axelar/vote/v1beta1/tx.proto",
            "proto/axelar/vote/exported/v1beta1/types.proto",
            "proto/axelar/permission/exported/v1beta1/types.proto",
            "proto/tofnd/grpc.proto",
            "proto/tofnd/multisig.proto"
        ],
        &["cosmos-sdk-go/proto/", "tendermint/proto/", "third_party/", "proto/tofnd/", "proto/"])?;
    Ok(())
}
