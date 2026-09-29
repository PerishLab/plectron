from pathlib import Path
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

font = TTFont('target/arabic/font.ttf', recalcTimestamp=False)
instantiateVariableFont(font, {'wght': 400, 'wdth': 100}, inplace=True)
names = {
    1: 'Plectron Arabic', 2: 'Regular', 3: 'PlectronArabic-Regular',
    4: 'Plectron Arabic Regular', 6: 'PlectronArabic-Regular',
    16: 'Plectron Arabic', 17: 'Regular',
}
for entry in font['name'].names:
    if entry.nameID in names:
        entry.string = names[entry.nameID].encode(entry.getEncoding())
font.save('apps/fonts/Arabic.ttf')
Path('apps/fonts/Arabic-OFL.txt').write_bytes(Path('target/arabic/OFL.txt').read_bytes())
