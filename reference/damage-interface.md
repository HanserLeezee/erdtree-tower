# Pre-HP damage interface

Verified by disassembling the installed Elden Ring 2.7.2.0 executable. The source fixture checks are in tests/test_damage_interface.py.

- CSEnemyDamageModule vtable: RVA 0x2a3c0d8. Module owner accessor at 0x43d2e0 returns [module+8].
- 0x4478f0 calculates damage. The incoming packet starts with five f32 attack-power components. Percentage-of-HP components are at 0x1e8..0x1f8.
- Native defense builder 0x6867a0 reads NpcParam physical/magic/fire/thunder/dark defenses at 0xf8, 0x100, 0x102, 0x104, 0x15e into the same five-component order. The five-channel calculator at 0x685cd0 consumes this order.
- 0x448703 converts the final scalar damage to i32 and stores it at packet+0x228.
- 0x449e10 calls 0x4478f0, then 0x4489f0. The latter checks death/no-damage conditions before the selected hook site at 0x448b34.
- At 0x448b34, r15 is the damage module, r14 the packet, and ebx pre-hit HP. The original instruction loads packet+0x228 into edx; 0x448b6c negates edx and 0x448b7b calls native HP change at 0x436b70.

The shim preserves flags, volatile integer registers, XMM0–5 and stack arguments. It never changes HP or death/throw flags. An optional pure Rust filter may only lower positive final damage to at least 1. With no filter, damage is unchanged. Raw attack power and percentage components identify mixed attacks; they are not presented as final per-element damage.

Set ERDTREE_TOWER_DAMAGE_TRACE=1 before starting the process for at most 256 DAMAGE_HIT samples. No filter is registered by default. Engine pointers are only read synchronously at the hook; subscribers receive values and must not access engine objects. Multiplayer authority and encounter membership must be enforced by the subscribing feature.

Static verification does not prove every gameplay path. Live pure-element, mixed-element, critical, status-damage and multiplayer tests remain required before enabling Adaptation rolls. Do not infer periodic-damage classification or host authority from the packet without verifying those paths.
