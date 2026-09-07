# ICHOR

On-chain program for atomic KEKBULL → ICHOR convert, plus the TypeScript client used to read and verify it.

## Mainnet identity

| | |
|---|---|
| Program | `DmnfCzQNbx4ZAWEwBoAKqTALgwuUXwYxZL1dRqXoZ1Md` |
| ICHOR mint | `61fHeSXmBuJMbqsYKVqgXifJDV8v9zzF7rFpysNT3Mq2` |
| KEKBULL mint | `CKFvy16Gd8kadtCmNyu9r91TYgyvrbJnsJey8McFpump` |
| Config | `JDKQUpipRB8TUQ8mFAuHZFxiuSSoRgboADM4fDbewFrF` |

Convert is published and **paused**. There is no genesis mint. Supply starts at 0 and only increases when someone burns KEKBULL.

There is no placeholder program id. Build with:

```bash
export KEKBULL_ICHOR_PROGRAM_ID=DmnfCzQNbx4ZAWEwBoAKqTALgwuUXwYxZL1dRqXoZ1Md
cargo test -p kekbull_ichor
```

To check the live ELF against a local SBF build, dump the program and compare hashes. A dump matching this tree is the verification; do not take a comment or a website number as the program.

## Client

`client/` builds unsigned instructions and parses Config / mint accounts. It does not send transactions and does not hold keys.

## Related repos

- [kekbull-governance](https://github.com/KEKBULL-ICHOR-DAO/kekbull-governance) — voting program
- [dao](https://github.com/KEKBULL-ICHOR-DAO/dao) — holder UI
- [website](https://github.com/KEKBULL-ICHOR-DAO/website) — kekbull.com
