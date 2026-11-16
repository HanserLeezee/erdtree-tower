"""Generate ABI-preserving forwarding classes from the locally supplied Steam SDK."""
import argparse
from pathlib import Path
import re

def generate(sdk):
    out = ['#pragma once', '#include "steam_api.h"', '#include "isteamnetworkingmessages.h"']
    for interface, filename in [('ISteamMatchmaking', 'isteammatchmaking.h'), ('ISteamNetworking', 'isteamnetworking.h'), ('ISteamNetworkingMessages', 'isteamnetworkingmessages.h')]:
        text = (sdk / filename).read_text(encoding='utf-8-sig')
        text = re.sub(r'/\*.*?\*/|//[^\n]*', '', text, flags=re.S)
        body = text.split('class ' + interface + '\n', 1)[-1] if 'class ' + interface + '\n' in text else text.split('class ' + interface + '\r\n', 1)[-1]
        if body == text:
            body = re.split(r'class\s+' + interface + r'\s*\{', text, maxsplit=1)[1]
        body = body.split('};', 1)[0]
        body = re.sub(r'STEAM_\w+\([^)]*\)', '', body)
        methods = re.findall(r'virtual\s+([^;{}]+?)\b(\w+)\s*\(([^;{}]*?)\)\s*=\s*0\s*;', body, re.S)
        methods = [m for m in methods if m[1] != 'CheckForPSNGameBootInvite']
        assert len(methods) == {'ISteamMatchmaking': 38, 'ISteamNetworking': 22, 'ISteamNetworkingMessages': 6}[interface], (interface, len(methods))
        out += [f'class Forward{interface} : public {interface} {{', 'protected:', f'    {interface}* real;', 'public:', f'    explicit Forward{interface}({interface}* value): real(value) {{}}']
        for ret, name, params in methods:
            ret = ' '.join(ret.split())
            params = re.sub(r'STEAM_\w+\([^)]*\)', '', params)
            params = ', '.join(p.split('=')[0].strip() for p in params.split(','))
            args = ', '.join(re.search(r'(\w+)\s*(?:\[[^]]*\])?$', p).group(1) for p in params.split(',')) if params.strip() else ''
            out.append(f'    {ret} {name}({params}) override {{ return real->{name}({args}); }}')
        out.append('};')
    return '\n'.join(out) + '\n'

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('sdk', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    args.output.write_text(generate(args.sdk), encoding='utf-8')
