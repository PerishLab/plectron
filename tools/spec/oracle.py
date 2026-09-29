import uharfbuzz as hb
import json
import sys
from pathlib import Path
strings=['سلام','السلام عليكم','مرحبا بالعالم','العَرَبِيَّة','لا لأ لإ لآ','ببب ب\u200cب ب\u200dب','بِسْمِ الله','فارسی اردو']
def shape(text, face, span):
    font=hb.Font(hb.Face(Path(f'apps/fonts/{face}.ttf').read_bytes()))
    size=1000 if face=='Arabic' else 2000
    font.scale=(size,size)
    b=hb.Buffer()
    b.flags=hb.BufferFlags.REMOVE_DEFAULT_IGNORABLES
    b.add_utf8(text.encode(),span[0],span[1]-span[0])
    b.direction='rtl' if face=='Arabic' else 'ltr'
    b.script='arab' if face=='Arabic' else 'latn'
    b.language='ar' if face=='Arabic' else 'en'
    hb.shape(font,b,{'kern':False,'calt':False})
    pen=0;glyphs=[]
    for info,pos in zip(b.glyph_infos,b.glyph_positions):
        glyphs.append([info.codepoint,pen+pos.x_offset,-pos.y_offset])
        pen+=pos.x_advance
    return {'text':text,'glyphs':glyphs,'advance':pen}

results=[]
if '--ranges' in sys.argv:
    samples=[('Lato-Regular','ffi',[0,1]),('Lato-Regular','ffi',[1,3]),('Lato-Regular','ffi',[0,2]),('Lato-Regular','Åffi',[3,6]),('Arabic','بلا',[0,2]),('Arabic','بلا',[2,4]),('Arabic','بلا',[4,6]),('Arabic','لَا',[0,4]),('Arabic','لَا',[4,6]),('Arabic','العَرَبِيَّة',[0,8]),('Arabic','بِبَبُ',[4,8])]
    for face,text,span in samples:
        result=shape(text,face,span)
        result.update(face=face,span=span)
        results.append(result)
else:
    for text in strings:
        results.append(shape(text,'Arabic',[0,len(text.encode())]))
print(json.dumps(results,ensure_ascii=False,indent=2))
