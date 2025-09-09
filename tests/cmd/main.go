package main

import (
	"context"
	"github.com/ethereum-optimism/optimism/kurtosis-devnet/pkg/util"
)

func main() {
	// Fix the traefik routes to access all ports via kurtosis reverse proxy
	err := util.SetReverseProxyConfig(context.Background())
	if err != nil {
		panic(err)
	}
}
