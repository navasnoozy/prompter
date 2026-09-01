import sys

with open('src/features/instructions/InstructionSidebar.tsx', 'r') as f:
    content = f.read()

content = content.replace(
    '<aside className="sidebar">',
    '<aside className="sidebar" data-tauri-drag-region>'
)

content = content.replace(
    '<div className="sidebar-navigation">',
    '<div className="sidebar-navigation" data-tauri-drag-region>'
)

with open('src/features/instructions/InstructionSidebar.tsx', 'w') as f:
    f.write(content)
print("patched InstructionSidebar.tsx")
