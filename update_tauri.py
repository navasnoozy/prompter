import json

with open('src-tauri/tauri.conf.json', 'r') as f:
    config = json.load(f)

for w in config['app']['windows']:
    if w['label'] == 'main':
        w['titleBarStyle'] = 'Overlay'
        w['hiddenTitle'] = True
        w['transparent'] = True

with open('src-tauri/tauri.conf.json', 'w') as f:
    json.dump(config, f, indent=2)

print("Updated tauri.conf.json")
