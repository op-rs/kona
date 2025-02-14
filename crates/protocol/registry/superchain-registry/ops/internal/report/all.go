package report

import (
	"context"
	"fmt"
	"time"

	"github.com/ethereum-optimism/optimism/op-deployer/pkg/deployer/artifacts"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum/go-ethereum/core"
	"github.com/ethereum/go-ethereum/ethclient"
	"github.com/ethereum/go-ethereum/rpc"
)

func ScanAll(
	ctx context.Context,
	rpcClient *rpc.Client,
	chainCfg *config.StagedChain,
	originalGenesis *core.Genesis,
) Report {
	var report Report
	var err error
	report.L1, err = ScanL1(
		ctx,
		rpcClient,
		*chainCfg.DeploymentTxHash,
		chainCfg.DeploymentL1ContractsVersion,
	)
	if err != nil {
		report.L1Err = err
	}

	startBlockHash := chainCfg.Genesis.L1.Hash
	startBlock, err := ethclient.NewClient(rpcClient).HeaderByHash(ctx, startBlockHash)
	if err != nil {
		report.L2Err = fmt.Errorf("error getting start block %s: %w", startBlockHash, err)
		return report
	}

	afacts, cleanup, err := artifacts.Download(ctx, chainCfg.DeploymentL2ContractsVersion, artifacts.NoopDownloadProgressor)
	if err != nil {
		report.L2Err = fmt.Errorf("error downloading L2 artifacts: %w", err)
		return report
	}
	defer func() {
		_ = cleanup()
	}()

	report.L2, err = ScanL2(
		startBlock,
		chainCfg,
		originalGenesis,
		afacts,
	)
	if err != nil {
		report.L2Err = err
	}

	report.GeneratedAt = time.Now()
	return report
}
