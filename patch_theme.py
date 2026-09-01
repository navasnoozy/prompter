import sys

with open('src/styles/theme.css', 'r') as f:
    content = f.read()

replacement = """  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
  /* --ink: #f0edf6;
  --muted: #9d97aa;
  --border: #34303f;
  --paper: #1b1922; */

  --ink: #000000;
  --muted: #555500;
  --border: #dddd00;
  --paper: #cccc00;

  /* Semantic surface tokens */"""

content = content.replace(
"""  --ink: #f0edf6;
  --muted: #9d97aa;
  --border: #34303f;
  --paper: #1b1922;

  /* Semantic surface tokens */""", replacement)


bg_replacement = """  color: var(--text-primary);
  /* background:
    radial-gradient(circle at 65% -10%, rgba(105, 74, 190, .2), transparent 30%),
    #121118; */
  background: yellow;
}"""

content = content.replace(
"""  color: var(--text-primary);
  background:
    radial-gradient(circle at 65% -10%, rgba(105, 74, 190, .2), transparent 30%),
    #121118;
}""", bg_replacement)

with open('src/styles/theme.css', 'w') as f:
    f.write(content)

print("patched theme.css")
