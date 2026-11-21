#pragma once
namespace ertransmogrify {
enum class effect_action { keep, apply, clear };
constexpr effect_action effect_update(bool changed, bool wanted, bool present) {
    if (wanted && (changed || !present)) return effect_action::apply;
    if (!wanted && present) return effect_action::clear;
    return effect_action::keep;
}
}
