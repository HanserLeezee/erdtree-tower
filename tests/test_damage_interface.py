import re
import struct
import unittest
from pathlib import Path
import pefile
import capstone

ROOT=Path(__file__).resolve().parents[1]
GAME=Path("F:/SteamLibrary/steamapps/common/ELDEN RING/Game/eldenring.exe")

class DamageInterface(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pe=pefile.PE(str(GAME),fast_load=True)
        cls.base=cls.pe.OPTIONAL_HEADER.ImageBase
        cls.dis=capstone.Cs(capstone.CS_ARCH_X86,capstone.CS_MODE_64)

    def instructions(self,rva,size):
        return [(i.mnemonic,i.op_str) for i in self.dis.disasm(self.pe.get_data(rva,size),self.base+rva)]

    def test_checked_signatures_match_current_executable(self):
        source=(ROOT/"native/src/damage.rs").read_text(encoding="utf-8")
        for address,values in re.findall(r"checked_function\((0x[0-9a-f]+),&\[([^]]+)\]",source):
            expected=bytes(int(x.strip(),0) for x in values.split(",") if x.strip())
            self.assertEqual(self.pe.get_data(int(address,16),len(expected)),expected)
        values=re.search(r"const SIGNATURE:&\[u8\]=&\[([^]]+)\]",source)[1]
        expected=bytes(int(x.strip(),0) for x in values.split(",") if x.strip())
        self.assertEqual(self.pe.get_data(0x448b34,len(expected)),expected)

    def test_final_damage_is_loaded_then_negated_before_native_hp_call(self):
        code=self.instructions(0x448b34,0x4c)
        self.assertEqual(code[0],("mov","edx, dword ptr [r14 + 0x228]"))
        self.assertIn(("neg","edx"),code)
        self.assertEqual(code[-1],("call",hex(self.base+0x436b70)))
        self.assertEqual(self.instructions(0x43d2e0,5),[("mov","rax, qword ptr [rcx + 8]"),("ret","")])
        self.assertIn(("mov","ebx, dword ptr [rax + 0x138]"),self.instructions(0x448ab6,0x13))

    def test_five_channel_array_matches_native_defense_order(self):
        code=self.instructions(0x686806,0x137)
        for offset in ["0xf8","0x100","0x102","0x104","0x15e"]:
            self.assertTrue(any(f"+ {offset}]" in op for _,op in code),offset)
        for destination in ["[rbx]","[rbx + 4]","[rbx + 8]","[rbx + 0xc]","[rbx + 0x10]"]:
            self.assertTrue(any(m=="movss" and op.startswith("dword ptr "+destination+",") for m,op in code),destination)
        self.assertIn(("movups","xmm0, xmmword ptr [r15]"),self.instructions(0x447df2,4))
        self.assertIn(("movss","xmm0, dword ptr [r15 + 0x10]"),self.instructions(0x447e0c,6))

if __name__=="__main__":
    unittest.main()
