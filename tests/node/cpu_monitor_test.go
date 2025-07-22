package node

import (
	"context"
	"math"
	"strconv"
	"strings"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"github.com/kurtosis-tech/kurtosis/api/golang/engine/lib/kurtosis_context"
	"github.com/stretchr/testify/require"
)

const (
	MAX_CPU_USAGE = 30
)

// GetCPUStats executes shell commands to get CPU usage statistics from a service
func GetCPUStats(t devtest.T, ctx context.Context, serviceName string) {

	kurtosisCtx, err := kurtosis_context.NewKurtosisContextFromLocalEngine()
	require.NoError(t, err, "failed to create kurtosis context")

	enclaves, err := kurtosisCtx.GetEnclaves(ctx)
	require.NoError(t, err, "failed to get enclaves")

	for enclave := range enclaves.GetEnclavesByName() {
		enclaveCtx, err := kurtosisCtx.GetEnclaveContext(ctx, enclave)
		require.NoError(t, err, "failed to get enclave context: %s", enclave)

		serviceCtx, err := enclaveCtx.GetServiceContext(serviceName)
		require.NoError(t, err, "failed to get service context: %s", serviceName)

		// CPU monitoring commands that work well in Linux containers. Gets the CPU usage percentage of the kona-node binary that runs in the service.
		cpuUsageCommand := []string{
			"sh", "-c", "ps aux | grep " + serviceName + " | head -1 | awk '{print $3}'",
		}

		exitCode, logs, err := serviceCtx.ExecCommand(cpuUsageCommand)

		require.NoError(t, err, "failed to execute command %s: %s", cpuUsageCommand, logs)

		trimmedLogs := strings.TrimSpace(logs)
		cpuUsageFloat, err := strconv.ParseFloat(trimmedLogs, 64)
		cpuUsage := int(math.Trunc(cpuUsageFloat))

		require.NoError(t, err, "failed to convert logs to int: %s", trimmedLogs)

		require.Equal(t, exitCode, int32(0), "exitCode: ", exitCode)
		require.LessOrEqual(t, cpuUsage, MAX_CPU_USAGE, "CPU usage is too high: %s, max allowed: %s", cpuUsage, MAX_CPU_USAGE)
	}
}

// Ensure that the CPU usage for a kona-node is less than the max allowed.
func TestKurtosisCPUMonitor(gt *testing.T) {
	t := devtest.ParallelT(gt)
	out := NewMixedOpKona(t)

	out.T.Gate().LessOrEqual(len(out.L2CLKonaNodes), 1, "expected at most one kona-node")

	opNode := out.L2CLOpNodes[0]
	konaNode := out.L2CLKonaNodes[0]

	// Wait for a few blocks to be produced before checking the CPU usage.
	dsl.CheckAll(t, konaNode.ReachedFn(types.LocalUnsafe, 40, 80), opNode.ReachedFn(types.LocalUnsafe, 40, 80))

	ctx := context.Background()

	GetCPUStats(t, ctx, konaNode.Escape().ID().Key())
}
