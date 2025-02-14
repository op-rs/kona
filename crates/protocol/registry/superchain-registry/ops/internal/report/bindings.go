package report

import (
	"context"
	"fmt"

	"github.com/ethereum/go-ethereum/common"
	"github.com/lmittmann/w3"
	"github.com/lmittmann/w3/module/eth"
	"github.com/lmittmann/w3/w3types"
)

var (
	deployedEventABI = w3.MustNewEvent(`Deployed(uint256 indexed, uint256 indexed, address indexed, bytes)`)

	versionABI = w3.MustNewFunc("version()", "string")

	guardianFnABI = w3.MustNewFunc("guardian()", "address")

	challengerFnABI = w3.MustNewFunc("challenger()", "address")

	ownerFnABI = w3.MustNewFunc("owner()", "address")

	gameTypeABI = w3.MustNewFunc("gameType()", "uint32")

	absolutePrestateFnABI = w3.MustNewFunc("absolutePrestate()", "bytes32")

	maxGameDepthABI = w3.MustNewFunc("maxGameDepth()", "uint256")

	splitDepthABI = w3.MustNewFunc("splitDepth()", "uint256")

	maxClockDurationABI = w3.MustNewFunc("maxClockDuration()", "uint64")

	clockExtensionABI = w3.MustNewFunc("clockExtension()", "uint64")

	gasLimitABI = w3.MustNewFunc("gasLimit()", "uint64")

	scalarABI = w3.MustNewFunc("scalar()", "uint256")

	overheadABI = w3.MustNewFunc("overhead()", "uint256")

	baseFeeScalarABI = w3.MustNewFunc("basefeeScalar()", "uint32")

	blobBaseFeeScalarABI = w3.MustNewFunc("blobbasefeeScalar()", "uint32")

	eip1559DenominatorABI = w3.MustNewFunc("eip1559Denominator()", "uint32")

	eip1559ElasticityABI = w3.MustNewFunc("eip1559Elasticity()", "uint32")

	isCustomGasTokenABI = w3.MustNewFunc("isCustomGasToken()", "bool")

	gasPayingTokenABI = w3.MustNewFunc("gasPayingToken()", "address,uint8")

	gasPayingTokenNameABI = w3.MustNewFunc("gasPayingTokenName()", "string")

	gasPayingTokenSymbolABI = w3.MustNewFunc("gasPayingTokenSymbol()", "string")

	deployOutputEvV0ABI = w3.MustNewFunc(`
dummy(
	address opChainProxyAdmin,
	address addressManager,
	address l1ERC721BridgeProxy,
	address systemConfigProxy,
	address optimismMintableERC20FactoryProxy,
	address l1StandardBridgeProxy,
	address l1CrossDomainMessengerProxy,
	address optimismPortalProxy,
	address disputeGameFactoryProxy,
	address anchorStateRegistryProxy,
	address anchorStateRegistryImpl,
	address faultDisputeGame,
	address permissionedDisputeGame,
	address delayedWETHPermissionedGameProxy,
	address delayedWETHPermissionlessGameProxy
)
`, "")
)

type BatchCall struct {
	To      common.Address
	Encoder func() ([]byte, error)
	Decoder func([]byte) error
}

func CallBatch(ctx context.Context, w3c *w3.Client, calls ...BatchCall) error {
	batchCalls := make([]w3types.RPCCaller, len(calls))
	rawOutputs := make([][]byte, len(calls))

	for i, call := range calls {
		input, err := call.Encoder()
		if err != nil {
			return fmt.Errorf("failed to encode input: %w", err)
		}

		msg := &w3types.Message{
			To:    &call.To,
			Input: input,
		}
		batchCalls[i] = eth.Call(msg, nil, nil).Returns(&rawOutputs[i])
	}

	if err := w3c.CallCtx(ctx, batchCalls...); err != nil {
		return fmt.Errorf("failed to perform batch call: %w", err)
	}

	for i, rawOutput := range rawOutputs {
		if err := calls[i].Decoder(rawOutput); err != nil {
			return fmt.Errorf("failed to decode output: %w", err)
		}
	}

	return nil
}
