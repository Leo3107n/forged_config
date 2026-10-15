# Forged Config

The treasury contains 10 SOL. Drain it without the legitimate authority.

Review `challenge/src/lib.rs` and implement the exploit in `solve/src/lib.rs`.

## Build

```bash
docker build --platform linux/amd64 -t forged-config-solve .
```

## Submit

```bash
docker run --rm --platform linux/amd64 forged-config-solve HOST PORT
```

For the local organizer instance, use `host.docker.internal 5003` on Docker Desktop.

The client derives a `fake-config` PDA under your solve program and includes it in the instruction.

## Program API

- `[0] WithdrawAll`: transfers the full treasury balance to the configured authority
- `[1, lamports_le_u64] WithdrawAmount`: transfers a requested amount
- `[2] GetTreasuryBalance`: returns the current treasury balance
- `[3] GetConfigAuthority`: returns the authority stored in a config account
- `[4, new_authority_pubkey] RotateConfigAuthority`: updates the stored authority
- `[5] GetCanonicalAddresses`: returns the canonical config and treasury addresses

## Account order

1. player, signer and writable
2. fake config PDA, writable
3. challenge treasury, writable
4. challenge program, executable
5. System Program, executable
