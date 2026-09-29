from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont
from pathlib import Path
for region in ['SC','JP','KR']:
    font = TTFont('target/fonts/'+region+'.ttf', recalcTimestamp=False)
    instantiateVariableFont(font, {'wght':400}, inplace=True)
    for entry in font['name'].names:
        names = {1:'Plectron CJK '+region,2:'Regular',3:'PlectronCJK'+region+'-Regular',4:'Plectron CJK '+region+' Regular',6:'PlectronCJK'+region+'-Regular',16:'Plectron CJK '+region,17:'Regular'}
        if entry.nameID in names:
            entry.string = names[entry.nameID].encode(entry.getEncoding())
    dest = 'apps/fonts/'+region+'.ttf'
    font.save(dest)
    print(region,Path(dest).stat().st_size,flush=True)
Path('apps/fonts/CJK-OFL.txt').write_bytes(Path('target/fonts/OFL.txt').read_bytes())
