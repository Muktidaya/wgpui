# SPDX-License-Identifier: Apache-2.0
"""Regenerate original WGPUI example fixtures (Apache-2.0); Python standard library only."""
from pathlib import Path
import struct,zlib,binascii,hashlib,json
out=Path(__file__).resolve().parent;out.mkdir(exist_ok=True)
def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',binascii.crc32(kind+data)&0xffffffff)
size=512;pixels=bytearray()
for y in range(size):
 pixels.append(0)
 for x in range(size):
  inside=48<=x<464 and 48<=y<464;accent=144<=x<368 and 144<=y<368
  pixels.extend((40,110,200,255) if accent else (24,35,55,220) if inside else (0,0,0,0))
png=b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',size,size,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(bytes(pixels),9))+chunk(b'IEND',b'');(out/'geometric-demo.png').write_bytes(png)
# Clear the LZW dictionary after each pair of literal pixels. All codes stay
# three bits wide, making this deliberately simple fixture encoder deterministic.
size=500;gif=bytearray(b'GIF89a'+struct.pack('<HHBBB',size,size,0x81,0,0));gif.extend(bytes([0,0,0,24,35,55,40,110,200,220,150,40]));gif.extend(b'\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00')
for frame in range(25):
 gif.extend(b'\x21\xf9\x04\x09\x04\x00\x00\x00');gif.extend(b'\x2c'+struct.pack('<HHHHB',0,0,size,size,0));gif.append(2)
 packed=bytearray();acc=0;bits=0;count=0
 def emit(code):
  global acc,bits
  acc|=code<<bits;bits+=3
  while bits>=8:packed.append(acc&255);acc>>=8;bits-=8
 for y in range(size):
  for x in range(size):
   if count%2==0:emit(4)
   value=0 if x<24 or x>=476 or y<24 or y>=476 else 3 if 30+frame*12<=x<130+frame*12 and 160<=y<340 else 2 if 80<=x<420 and 80<=y<420 else 1
   emit(value);count+=1
 emit(5)
 if bits:packed.append(acc&255)
 for offset in range(0,len(packed),255):piece=packed[offset:offset+255];gif.append(len(piece));gif.extend(piece)
 gif.append(0)
gif.append(0x3b);(out/'geometric-animation.gif').write_bytes(gif)
(out/'circle-arrow-demo.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M12 5A5 5 0 1 0 13 9M12 2v3H9" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg>\n')
(out/'colored-shapes-demo.svg').write_text('<svg xmlns="http://www.w3.org/2000/svg" width="480" height="480" viewBox="0 0 480 480"><rect x="40" y="40" width="170" height="170" rx="24" fill="#286ec8"/><circle cx="330" cy="125" r="85" fill="#dc9628"/><path d="M40 420 125 250 210 420Z" fill="#2f996f"/><rect x="245" y="250" width="170" height="170" rx="85" fill="#b14c7d"/></svg>\n')
rows=[{'file':p.name,'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in sorted(out.iterdir()) if p.name in ['geometric-demo.png','geometric-animation.gif','circle-arrow-demo.svg','colored-shapes-demo.svg']]
(out/'fixture-inventory.json').write_text(json.dumps({'license':'Apache-2.0','files':rows},indent=2)+'\n')
