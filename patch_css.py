import sys

with open('src/styles/foundation.css', 'r') as f:
    content = f.read()

replacement = """:root {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  color: #20202a;
  /* TEMPORARY YELLOW DEVELOPMENT COLORS */
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
  --backdrop-bg: rgba(255, 255, 0, 0.48);
}"""

content = content.replace(
    """:root {
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  color: #20202a;
  background: #f4f2f8;
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
  --backdrop-bg: rgba(24, 20, 34, 0.48);
}""", replacement)

shell_replacement = """.app-shell {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 270px minmax(0, 1fr);
  /* background:
    radial-gradient(circle at 65% -10%, rgba(179, 159, 237, 0.16), transparent 30%),
    #f7f6fa; */
  background: yellow;
}"""

content = content.replace(
    """.app-shell {
  min-height: 100vh;
  display: grid;
  grid-template-columns: 270px minmax(0, 1fr);
  background:
    radial-gradient(circle at 65% -10%, rgba(179, 159, 237, 0.16), transparent 30%),
    #f7f6fa;
}""", shell_replacement)


# Also handle dark mode media query
dark_replacement = """@media (prefers-color-scheme: dark) {
  :root {
    /* TEMPORARY YELLOW DEVELOPMENT COLORS */
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
    --backdrop-bg: rgba(200, 200, 0, 0.65);
  }"""

content = content.replace(
    """@media (prefers-color-scheme: dark) {
  :root {
    --ink: #f0edf6;
    --muted: #9d97aa;
    --border: #34303f;
    --paper: #1b1922;
    --purple: #8c6dfd;
    --purple-dark: #7452e6;
    --purple-soft: #282038;
    --backdrop-bg: rgba(10, 8, 16, 0.65);
  }""", dark_replacement)


with open('src/styles/foundation.css', 'w') as f:
    f.write(content)
print("patched foundation.css")
