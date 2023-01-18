use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "axelar-core/proto/axelar/multisig/exported/v1beta1/types.proto",
            "axelar-core/proto/axelar/multisig/v1beta1/events.proto",
            "axelar-core/proto/axelar/multisig/v1beta1/tx.proto",
            "axelar-core/proto/axelar/multisig/v1beta1/types.proto",
            "axelar-core/proto/axelar/vote/exported/v1beta1/types.proto",
            "axelar-core/proto/axelar/vote/v1beta1/tx.proto",
            "axelar-core/proto/axelar/evm/v1beta1/events.proto",
            "axelar-core/proto/axelar/evm/v1beta1/types.proto",
            "proto/tofnd/grpc.proto",
            "proto/tofnd/multisig.proto",
        ],
        &[
            "axelar-core/proto/",
            "third_party/cosmos-sdk/proto/",
            "third_party/tendermint/proto/",
            "third_party/proto/",
            "proto/tofnd/",
        ],
    )?;
    Ok(())
}
