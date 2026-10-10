mos6502
use tests from :
https://github.com/SingleStepTests/65x02

This is a 6502 emulator. By default it builds the NES 2A03 variant, which has
no decimal mode. Enable the `decimal` feature to build a standard NMOS 6502,
where ADC/SBC (and the unofficial RRA/ISC/ARR) honour the decimal flag:

```toml
mos6502 = { path = "...", features = ["decimal"] }
```

The tests read `65x02/nes6502/v1` by default and `65x02/6502/v1` with
`--features decimal`.