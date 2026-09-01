import sys

def revert_file(filepath, target, replacement):
    with open(filepath, 'r') as f:
        content = f.read()
    if target in content:
        content = content.replace(target, replacement)
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Reverted {filepath}")
    else:
        print(f"Target not found in {filepath}")

# 1. Foundation CSS Light
target_found_light = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* background: #f4f2f8; */
  background: yellow;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  
  /* --ink: #20202a;
  --muted: #6f6a79;
  --border: #e8e4ef;
  --paper: #ffffff;
  --purple: #6e51d9;
  --purple-dark: #5638c0;
  --purple-soft: #f1edff;
  --backdrop-bg: rgba(24, 20, 34, 0.48); */

  --ink: #20202a;
  --muted: #555500;
  --border: #dddd00;
  --paper: #ffff66;
  --purple: #aa8800;
  --purple-dark: #886600;
  --purple-soft: #ffffaa;
  --backdrop-bg: rgba(255, 255, 0, 0.48);"""
  
repl_found_light = """  background: #f4f2f8;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  --ink: #20202a;
  --muted: #6f6a79;
  --border: #e8e4ef;
  --paper: #ffffff;
  --purple: #6e51d9;
  --purple-dark: #5638c0;
  --purple-soft: #f1edff;
  --backdrop-bg: rgba(24, 20, 34, 0.48);"""

revert_file('src/styles/foundation.css', target_found_light, repl_found_light)

# 2. Foundation CSS Shell
target_shell = """  /* background:
    radial-gradient(circle at 65% -10%, rgba(179, 159, 237, 0.16), transparent 30%),
    #f7f6fa; */
  background: yellow;"""
repl_shell = """  background:
    radial-gradient(circle at 65% -10%, rgba(179, 159, 237, 0.16), transparent 30%),
    #f7f6fa;"""
revert_file('src/styles/foundation.css', target_shell, repl_shell)

# 3. Foundation CSS Dark
target_found_dark = """    /* TEMPORARY YELLOW DEVELOPMENT COLORS */
    /* --ink: #f0edf6;
    --muted: #9d97aa;
    --border: #34303f;
    --paper: #1b1922;
    --purple: #8c6dfd;
    --purple-dark: #7452e6;
    --purple-soft: #282038;
    --backdrop-bg: rgba(10, 8, 16, 0.65); */
    
    --ink: #000000;
    --muted: #555500;
    --border: #dddd00;
    --paper: #cccc00;
    --purple: #886600;
    --purple-dark: #664400;
    --purple-soft: #aaaa00;
    --backdrop-bg: rgba(200, 200, 0, 0.65);"""
    
repl_found_dark = """    --ink: #f0edf6;
    --muted: #9d97aa;
    --border: #34303f;
    --paper: #1b1922;
    --purple: #8c6dfd;
    --purple-dark: #7452e6;
    --purple-soft: #282038;
    --backdrop-bg: rgba(10, 8, 16, 0.65);"""
revert_file('src/styles/foundation.css', target_found_dark, repl_found_dark)

# 4. Theme CSS Variables
target_theme_vars = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* --ink: #f0edf6;
  --muted: #9d97aa;
  --border: #34303f;
  --paper: #1b1922; */

  --ink: #000000;
  --muted: #555500;
  --border: #dddd00;
  --paper: #cccc00;"""
repl_theme_vars = """  --ink: #f0edf6;
  --muted: #9d97aa;
  --border: #34303f;
  --paper: #1b1922;"""
revert_file('src/styles/theme.css', target_theme_vars, repl_theme_vars)

# 5. Theme CSS Background
target_theme_bg = """  /* background:
    radial-gradient(circle at 65% -10%, rgba(105, 74, 190, .2), transparent 30%),
    #121118; */
  background: yellow;"""
repl_theme_bg = """  background:
    radial-gradient(circle at 65% -10%, rgba(105, 74, 190, .2), transparent 30%),
    #121118;"""
revert_file('src/styles/theme.css', target_theme_bg, repl_theme_bg)

# 6. Instructions CSS Text
target_inst_text = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* color: #f7f4ff; */
  color: black;"""
repl_inst_text = """  color: #f7f4ff;"""
revert_file('src/features/instructions/instructions.css', target_inst_text, repl_inst_text)

# 7. Instructions CSS bg
target_inst_bg = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* background:
    radial-gradient(circle at 10% 5%, rgba(145, 114, 236, 0.32), transparent 25%),
    linear-gradient(175deg, #2c2348 0%, #1d1930 62%, #171523 100%); */
  background: yellow;"""
repl_inst_bg = """  background:
    radial-gradient(circle at 10% 5%, rgba(145, 114, 236, 0.32), transparent 25%),
    linear-gradient(175deg, #2c2348 0%, #1d1930 62%, #171523 100%);"""
revert_file('src/features/instructions/instructions.css', target_inst_bg, repl_inst_bg)

# 8. Sidebar Drag Region
with open('src/features/instructions/InstructionSidebar.tsx', 'r') as f:
    content = f.read()
content = content.replace('<aside className="sidebar" data-tauri-drag-region>', '<aside className="sidebar">')
content = content.replace('<div className="sidebar-navigation" data-tauri-drag-region>', '<div className="sidebar-navigation">')
with open('src/features/instructions/InstructionSidebar.tsx', 'w') as f:
    f.write(content)
print("Reverted InstructionSidebar.tsx")

# 9. Tauri Conf
import json
with open('src-tauri/tauri.conf.json', 'r') as f:
    config = json.load(f)

for w in config['app']['windows']:
    if w['label'] == 'main':
        w.pop('titleBarStyle', None)
        w.pop('hiddenTitle', None)
        w.pop('transparent', None)

with open('src-tauri/tauri.conf.json', 'w') as f:
    json.dump(config, f, indent=2)
print("Reverted tauri.conf.json")
