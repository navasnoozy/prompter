import sys

with open('src/features/instructions/instructions.css', 'r') as f:
    content = f.read()

replacement = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* color: #f7f4ff; */
  color: black;"""

content = content.replace("  color: #f7f4ff;", replacement)

with open('src/features/instructions/instructions.css', 'w') as f:
    f.write(content)
print("patched instructions.css text color")
