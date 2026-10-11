"""Wrap a diagram SVG in a navigable viewer: guided tour, click-to-focus, search.
Usage: python3 make_tour.py diagram.svg tour.json out.html"""
import json, sys
svg, tour, out = sys.argv[1:4]
html = open(__file__.replace("make_tour.py", "tour_template.html")).read()
html = html.replace("/*SVG*/", open(svg).read()).replace("/*TOUR*/", json.dumps(json.load(open(tour))))
open(out, "w").write(html)
print("wrote", out)
