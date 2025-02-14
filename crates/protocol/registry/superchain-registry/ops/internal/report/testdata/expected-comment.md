# Standards Compliance Report

## L1 Deployment

**Release**: `op-contracts/v1.6.0`

**Deployment Transaction Hash**: [0x0193884dc77ba74ca9ae23079edf91b81d49b0243c486fab5bbecd415d2dad68](https://sepolia.etherscan.io/tx/0x0193884dc77ba74ca9ae23079edf91b81d49b0243c486fab5bbecd415d2dad68)

<details open>

<summary>Semvers</summary>

| | Contract | Std Version | Got Version |
|---|----------|-------------|-------------|
| ✅ | SystemConfig | 2.2.0 | 2.2.0 |
| ⚠️ | PermissionedDisputeGame | 1.3.0 | 1.3.1-beta.3 |
| ✅ | OptimismPortal | 3.10.0 | 3.10.0 |
| ⚠️ | AnchorStateRegistry | 2.0.0 | 2.0.1-beta.3 |
| ✅ | DisputeGameFactory | 1.0.0 | 1.0.0 |
| ✅ | L1CrossDomainMessenger | 2.3.0 | 2.3.0 |
| ✅ | L1StandardBridge | 2.1.0 | 2.1.0 |
| ✅ | L1ERC721Bridge | 2.1.0 | 2.1.0 |
| ✅ | OptimismMintableERC20Factory | 1.9.0 | 1.9.0 |

</details>

<details open>

<summary>Ownership</summary>

| | Contract | Std Address | Got Address |
|---|----------|-------------|-------------|
| ✅ | Guardian | `0x7a50f00e8d05b95f98fe38d8bee366a7324dcf7e` | `0x7a50f00e8D05b95F98fE38d8BeE366a7324dCf7E` |
| ✅ | Challenger | `0xfd1d2e729ae8eee2e146c033bf4400fe75284301` | `0xfd1D2e729aE8eEe2E146c033bf4400fE75284301` |
| ✅ | ProxyAdminOwner | `0x1eb2ffc903729a0f03966b917003800b145f56e2` | `0x1Eb2fFc903729a0F03966B917003800b145F56E2` |

</details>

<details open>

<summary>Permissioned Proofs</summary>

| | Contract           | Std Param                                    | Got Param                                    |
|---|-------------------|----------------------------------------------|----------------------------------------------|
| ✅ | GameType | `1` | `1` |
| ✅ | AbsolutePrestate | `0x038512e02c4c3f7bdaec27d00edf55b7155e0905301e1a88083e4e0a6764d54c` | `0x038512e02c4c3f7bdaec27d00edf55b7155e0905301e1a88083e4e0a6764d54c` |
| ✅ | MaxGameDepth | `73` | `73` |
| ✅ | SplitDepth | `30` | `30` |
| ✅ | MaxClockDuration | `302400` | `302400` |
| ✅ | ClockExtension | `10800` | `10800` |

</details>

## L2 Deployment

**⚠️ Genesis does not match standard.** The state diff is listed below:

```diff
+0x0000000000000000000000000000000000000123
+code:0x010203
+balance:100
+nonce:1
+storage:
+  0x0000000000000000000000000000000000000000000000000000000000000456:0x0000000000000000000000000000000000000000000000000000000000000789
```

<small>Report generated on 1970-01-01T00:20:34Z for commit `1234567890abcdef`</small>

<!--- Report Magic V1 -->
