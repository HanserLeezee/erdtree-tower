#include "../native/vendor/tower-transmog/src/transmog_effect_action.hpp"
using enum ertransmogrify::effect_action;
static_assert(ertransmogrify::effect_update(false,true,false)==apply);
static_assert(ertransmogrify::effect_update(false,true,true)==keep);
static_assert(ertransmogrify::effect_update(false,false,false)==keep);
static_assert(ertransmogrify::effect_update(true,false,true)==clear);
static_assert(ertransmogrify::effect_update(true,true,true)==apply);
int main() {}
