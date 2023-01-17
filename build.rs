use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(&[
            "proto/axelar/vote/v1beta1/tx.proto",
            "proto/axelar/vote/exported/v1beta1/types.proto",
            "proto/axelar/permission/exported/v1beta1/types.proto",
            "proto/tofnd/grpc.proto",
            "proto/tofnd/multisig.proto"
        ],
        &[
            "third_party/cosmos-sdk/proto/",
            "third_party/tendermint/proto/",
            "third_party/proto/",
            "proto/tofnd/",
            "proto/"
        ],
    )?;
    Ok(())
}
