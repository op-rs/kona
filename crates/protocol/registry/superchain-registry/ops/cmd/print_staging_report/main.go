package main

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path"
	"time"

	"github.com/ethereum-optimism/superchain-registry/ops/internal/config"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/gh"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/manage"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/output"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/paths"
	"github.com/ethereum-optimism/superchain-registry/ops/internal/report"
	"github.com/ethereum-optimism/superchain-registry/validation"
	"github.com/ethereum/go-ethereum/rpc"
	"github.com/google/go-github/v68/github"
	"github.com/urfave/cli/v2"
)

var (
	SepoliaRPCURLFlag = &cli.StringFlag{
		Name:     "sepolia-rpc-url",
		Usage:    "The URL of the Sepolia RPC endpoint.",
		EnvVars:  []string{"SEPOLIA_RPC_URL"},
		Required: true,
	}
	MainnetRPCURLFlag = &cli.StringFlag{
		Name:     "mainnet-rpc-url",
		Usage:    "The URL of the mainnet RPC endpoint.",
		EnvVars:  []string{"MAINNET_RPC_URL"},
		Required: true,
	}
	PRURLFlag = &cli.StringFlag{
		Name:     "pr-url",
		Usage:    "URL to the pull request.",
		EnvVars:  []string{"PR_NUMBER", "CIRCLE_PULL_REQUEST"},
		Required: true,
	}
	GitSHAFlag = &cli.StringFlag{
		Name:     "git-sha",
		Usage:    "The git SHA of the commit being reported on.",
		EnvVars:  []string{"GIT_SHA", "CIRCLE_SHA1"},
		Required: true,
	}
	GithubTokenFlag = &cli.StringFlag{
		Name:     "github-token",
		Usage:    "The GitHub token to use for API requests.",
		EnvVars:  []string{"GITHUB_TOKEN"},
		Required: true,
	}
	GithubRepoFlag = &cli.StringFlag{
		Name:     "github-repo",
		Usage:    "The GitHub repository to comment on.",
		EnvVars:  []string{"GITHUB_REPO"},
		Value:    "ethereum-optimism/superchain-registry",
		Required: true,
	}
)

func main() {
	app := &cli.App{
		Name:  "print-staging-report",
		Usage: "Prints a standards compliance report for the Standard Blockspace Charter.",
		Flags: []cli.Flag{
			SepoliaRPCURLFlag,
			MainnetRPCURLFlag,
			PRURLFlag,
			GitSHAFlag,
			GithubTokenFlag,
			GithubRepoFlag,
		},
		Action: PrintStagingReport,
	}
	if err := app.Run(os.Args); err != nil {
		output.WriteStderr("%v", err)
		os.Exit(1)
	}
}

func PrintStagingReport(cliCtx *cli.Context) error {
	prURL := cliCtx.String(PRURLFlag.Name)
	gitSHA := cliCtx.String(GitSHAFlag.Name)
	githubToken := cliCtx.String(GithubTokenFlag.Name)
	githubRepo := cliCtx.String(GithubRepoFlag.Name)

	wd, err := paths.FindRepoRoot()
	if err != nil {
		return fmt.Errorf("failed to get working directory: %w", err)
	}

	if err := paths.RequireRoot(wd); err != nil {
		return fmt.Errorf("root directory error: %w", err)
	}

	chainCfg, err := manage.StagedChainConfig(wd)
	if errors.Is(err, manage.ErrNoStagedConfig) {
		output.WriteOK("no staged chain config found, exiting")
		return nil
	}
	if err != nil {
		return fmt.Errorf("failed to get staged chain config: %w", err)
	}

	genesisFilename := chainCfg.ShortName + ".json.zst"
	originalGenesis, err := manage.ReadGenesis(wd, path.Join(paths.StagingDir(wd), genesisFilename))
	if err != nil {
		return fmt.Errorf("failed to read genesis: %w", err)
	}

	if chainCfg.DeploymentTxHash == nil {
		return fmt.Errorf("deployment tx hash is required")
	}

	contractsVersion := validation.Semver(chainCfg.DeploymentL1ContractsVersion.Tag)
	stdPrestate := validation.StandardPrestates.StablePrestate()
	stdVersions := validation.StandardVersionsMainnet[contractsVersion]

	var stdConfigs validation.ConfigParams
	var stdRoles validation.RolesConfig
	var l1RPCURL string
	switch chainCfg.Superchain {
	case config.MainnetSuperchain:
		stdConfigs = validation.StandardConfigParamsMainnet
		stdRoles = validation.StandardConfigRolesMainnet
		l1RPCURL = cliCtx.String(MainnetRPCURLFlag.Name)
	case config.SepoliaSuperchain:
		stdConfigs = validation.StandardConfigParamsSepolia
		stdRoles = validation.StandardConfigRolesSepolia
		l1RPCURL = cliCtx.String(SepoliaRPCURLFlag.Name)
	default:
		return fmt.Errorf("unsupported superchain: %s", chainCfg.Superchain)
	}

	rpcClient, err := rpc.Dial(l1RPCURL)
	if err != nil {
		return fmt.Errorf("failed to dial RPC client: %w", err)
	}

	var params validation.ConfigParams
	if err := paths.ReadTOMLFile(paths.ValidationsFile(wd, string(chainCfg.Superchain)), &params); err != nil {
		return fmt.Errorf("failed to read standard params: %w", err)
	}

	ctx, cancel := context.WithTimeout(cliCtx.Context, 5*time.Minute)
	defer cancel()

	allReport := report.ScanAll(ctx, rpcClient, chainCfg, originalGenesis)
	output.WriteOK("scanned L1 and L2")

	ghClient := github.NewClient(nil).WithAuthToken(githubToken)

	currUser, _, err := ghClient.Users.Get(ctx, "")
	if err != nil {
		return fmt.Errorf("failed to get authenticated GitHub user: %w", err)
	}

	comment, err := report.RenderComment(
		&allReport,
		stdConfigs,
		stdRoles,
		stdPrestate,
		stdVersions,
		gitSHA,
	)
	if err != nil {
		return fmt.Errorf("failed to render comment: %w", err)
	}

	prNum, err := gh.GetPRNumberFromURL(prURL)
	if err != nil {
		return fmt.Errorf("failed to get PR number from URL: %w", err)
	}

	commenter := gh.NewGithubCommenter(githubRepo, ghClient)

	existingComment, err := commenter.FindComment(
		ctx,
		prNum,
		gh.CommentContaining(report.CommentMagic),
		gh.CommentFrom(currUser.GetLogin()),
	)
	if err != nil {
		return fmt.Errorf("failed to find existing comment: %w", err)
	}

	if existingComment == nil {
		if err := commenter.PostComment(ctx, prNum, comment); err != nil {
			return fmt.Errorf("failed to post comment: %w", err)
		}
		output.WriteOK("posted comment on PR %d", prNum)
	} else {
		if err := commenter.EditComment(ctx, existingComment.GetID(), comment); err != nil {
			return fmt.Errorf("failed to edit comment: %w", err)
		}
		output.WriteOK("edited comment on PR %d", prNum)
	}

	return nil
}
