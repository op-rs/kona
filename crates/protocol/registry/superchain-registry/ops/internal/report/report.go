package report

import (
	"fmt"
	"math/big"
	"strings"
	"time"

	"github.com/ethereum/go-ethereum/common"
)

type L1SemversReport struct {
	SystemConfig                       string
	PermissionedDisputeGame            string
	OptimismPortal                     string
	AnchorStateRegistry                string
	DelayedWETHPermissionedDisputeGame string
	DisputeGameFactory                 string
	L1CrossDomainMessenger             string
	L1StandardBridge                   string
	L1ERC721Bridge                     string
	OptimismMintableERC20Factory       string
}

type L1OwnershipReport struct {
	Guardian        common.Address
	Challenger      common.Address
	ProxyAdminOwner common.Address
}

type L1FDGReport struct {
	GameType         uint32
	AbsolutePrestate common.Hash
	MaxGameDepth     uint64
	SplitDepth       uint64
	MaxClockDuration uint64
	ClockExtension   uint64
}

type L1ProofsReport struct {
	Permissioned   L1FDGReport
	Permissionless *L1FDGReport
}

type L1SystemConfigReport struct {
	GasLimit               uint64
	Scalar                 *big.Int
	Overhead               *big.Int
	BaseFeeScalar          uint32
	BlobBaseFeeScalar      uint32
	EIP1559Denominator     uint32
	EIP1559Elasticity      uint32
	IsGasPayingToken       bool
	GasPayingToken         common.Address
	GasPayingTokenDecimals uint8
	GasPayingTokenName     string
	GasPayingTokenSymbol   string
}

type L1Report struct {
	Release           string
	DeploymentChainID uint64
	DeploymentTxHash  common.Hash
	Semvers           L1SemversReport
	Ownership         L1OwnershipReport
	Proofs            L1ProofsReport
	SystemConfig      L1SystemConfigReport
}

type L2Report struct {
	Release             string
	ChainID             common.Hash
	ProvidedGenesisHash common.Hash
	StandardGenesisHash common.Hash
	AccountDiffs        []AccountDiff
}

type StorageDiff struct {
	Key common.Hash

	Added   bool
	Removed bool

	OldValue common.Hash
	NewValue common.Hash
}

type AccountDiff struct {
	Address common.Address

	Added   bool
	Removed bool

	CodeChanged    bool
	BalanceChanged bool
	NonceChanged   bool

	OldCode    []byte
	OldBalance *big.Int
	OldNonce   uint64

	NewCode    []byte
	NewBalance *big.Int
	NewNonce   uint64

	// Storage changes as a list of modifications
	StorageChanges []StorageDiff
}

func (a AccountDiff) AsMarkdown() string {
	if a.Added || a.Removed {
		return a.fullDiff()
	}

	var builder strings.Builder

	builder.WriteString(fmt.Sprintf("%s\n", a.Address.Hex()))

	if a.CodeChanged {
		builder.WriteString(fmt.Sprintf("-code:0x%x\n", a.OldCode))
		builder.WriteString(fmt.Sprintf("+code:0x%x\n", a.NewCode))
	}

	if a.BalanceChanged {
		builder.WriteString(fmt.Sprintf("-balance:%s\n", a.OldBalance))
		builder.WriteString(fmt.Sprintf("+balance:%s\n", a.NewBalance))
	}

	if a.NonceChanged {
		builder.WriteString(fmt.Sprintf("-nonce:%d\n", a.OldNonce))
		builder.WriteString(fmt.Sprintf("+nonce:%d\n", a.NewNonce))
	}

	if len(a.StorageChanges) > 0 {
		builder.WriteString("storage:\n")
		for _, diff := range a.StorageChanges {
			if diff.Added {
				builder.WriteString(fmt.Sprintf("+  %s:0x%x\n", diff.Key.Hex(), diff.NewValue))
			} else if diff.Removed {
				builder.WriteString(fmt.Sprintf("-  %s:0x%x\n", diff.Key.Hex(), diff.OldValue))
			} else {
				builder.WriteString(fmt.Sprintf("-  %s:0x%x\n", diff.Key.Hex(), diff.OldValue))
				builder.WriteString(fmt.Sprintf("+  %s:0x%x\n", diff.Key.Hex(), diff.NewValue))
			}
		}
	}

	return builder.String()
}

func (a AccountDiff) fullDiff() string {
	var prefix string
	var code []byte
	var balance *big.Int
	var nonce uint64

	if a.Added {
		prefix = "+"
		code = a.NewCode
		balance = a.NewBalance
		nonce = a.NewNonce
	} else {
		prefix = "-"
		code = a.OldCode
		balance = a.OldBalance
		nonce = a.OldNonce
	}

	var builder strings.Builder
	builder.WriteString(fmt.Sprintf("%s%s\n", prefix, a.Address))
	builder.WriteString(fmt.Sprintf("%scode:0x%x\n", prefix, code))
	builder.WriteString(fmt.Sprintf("%sbalance:%s\n", prefix, balance))
	builder.WriteString(fmt.Sprintf("%snonce:%d\n", prefix, nonce))

	if len(a.StorageChanges) > 0 {
		builder.WriteString(fmt.Sprintf("%sstorage:\n", prefix))
	}

	for _, diff := range a.StorageChanges {
		val := diff.NewValue
		if a.Removed {
			val = diff.OldValue
		}
		builder.WriteString(fmt.Sprintf("%s  %s:0x%x\n", prefix, diff.Key, val))
	}
	return builder.String()
}

type Report struct {
	L1          *L1Report
	L1Err       error
	L2          *L2Report
	L2Err       error
	GeneratedAt time.Time
}
