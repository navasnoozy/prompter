import sys

with open('src/features/instructions/instructions.css', 'r') as f:
    content = f.read()

replacement = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* background:
    radial-gradient(circle at 10% 5%, rgba(145, 114, 236, 0.32), transparent 25%),
    linear-gradient(175deg, #2c2348 0%, #1d1930 62%, #171523 100%); */
  background: yellow;"""

content = content.replace(
"""  background:
    radial-gradient(circle at 10% 5%, rgba(145, 114, 236, 0.32), transparent 25%),
    linear-gradient(175deg, #2c2348 0%, #1d1930 62%, #171523 100%);""", replacement)

with open('src/features/instructions/instructions.css', 'w') as f:
    f.write(content)
print("patched instructions.css")
