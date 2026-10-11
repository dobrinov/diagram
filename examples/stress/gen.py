# Generates grid-like diagrams via pinned coordinates (workaround: no native primitives)
D=22  # px per day
X0=200; Y0=60; ROW=50

def gantt():
    L=["title: Atlas launch plan (Gantt, faked with pinned boxes)","layout: grid","routing: orthogonal",""]
    tasks=[ # id,label,lane,start,days,pct,color
      ("t1","Data model v2",0,0,6,100,"green"),
      ("t2","Billing API",0,6,10,70,"blue"),
      ("t3","Onboarding UI",1,3,12,40,"blue"),
      ("t4","Bank feeds",2,8,8,0,"red"),
      ("t5","Reports API",2,16,8,20,"blue"),
      ("t6","QA hardening",3,18,8,0,"gray"),
    ]
    lanes=["Backend","Frontend","Integrations","QA"]
    for i,l in enumerate(lanes):
        L.append(f"lane{i}: {l} [text, x: 20, y: {Y0+i*ROW+4}, w: 160]")
    for w in range(5):
        L.append(f"wk{w}: W{41+w} [text, x: {X0+w*7*D}, y: 20, w: {7*D}]")
    for t,lab,lane,s,d,pct,c in tasks:
        L.append(f"{t}: {lab} [rounded, {c}, detail: {pct}%, x: {X0+s*D}, y: {Y0+lane*ROW}, w: {d*D}, h: 40]")
    L.append(f"m1: Beta [diamond, orange, x: {X0+20*D}, y: {Y0+4*ROW}, w: 60, h: 40]")
    L.append(f"m2: Launch [diamond, red, x: {X0+28*D}, y: {Y0+4*ROW}, w: 70, h: 40]")
    L.append(f"today: Today [pill, red, x: {X0+9*D-30}, y: {Y0+5*ROW}, w: 60, h: 26]")
    L+=["","t1 -> t2","t2 -> t5","t4 -> t5: blocks [red, dashed]","t3 -> m1","t5 -> t6 -> m2"]
    return "\n".join(L)

def risk():
    C=60; G=8
    col={2:"green",3:"green",4:"green",5:"yellow",6:"yellow",8:"yellow",9:"orange",10:"orange",12:"orange",15:"red",16:"red",20:"red",25:"red"}
    def c(s):
        return "green" if s<=4 else "yellow" if s<=8 else "orange" if s<=12 else "red"
    L=["title: Risk heat-map (faked with 25 pinned boxes)",""]
    for li in range(1,6):
        L.append(f"ly{li}: {li} [text, x: 20, y: {40+(5-li)*(C+G)+18}, w: 30]")
        for ii in range(1,6):
            L.append(f"cell{li}{ii}: \" \" [box, {c(li*ii)}, x: {60+(ii-1)*(C+G)}, y: {40+(5-li)*(C+G)}, w: {C}, h: {C}]")
    for ii in range(1,6):
        L.append(f"ix{ii}: {ii} [text, x: {60+(ii-1)*(C+G)+15}, y: {40+5*(C+G)}, w: 30]")
    L.append(f"axl: Likelihood ↑ / Impact → [text, x: 60, y: {40+5*(C+G)+30}, w: 300]")
    risks=[(1,"Headless runs can't answer prompts",4,3,"Open · Deyan"),(2,"Spec Kit format drift",3,3,"Open · Claude"),(3,"Self-hosting loop breaks",2,4,"Open · Deyan"),(4,"Broad agent permissions",2,4,"Open · Deyan"),(5,"Token cost of headless runs",3,2,"Open · Deyan")]
    for n,lab,l,i,meta in risks:
        off=(n%2)*24
        L.append(f"dot{n}: {n} [circle, #222222, x: {60+(i-1)*(C+G)+8+off}, y: {40+(5-l)*(C+G)+16}, w: 28, h: 28]")
        L.append(f"r{n}: {n}. {lab} [rounded, detail: L{l}×I{i}={l*i} · {meta}, x: 420, y: {40+(n-1)*66}, w: 340, h: 56]")
    return "\n".join(L)

def raci():
    people=["Deyan (PM)","Claude Code","Ana (Eng)","Mia (QA)"]
    acts=[("Constitution & principles","ARCI"),("Feature specs","ARCI"),("Technical plans","ARRC"),("Implementation","ARRI"),("Test plans","ACCR"),("Release","ARIC")]
    colr={"R":"red","A":"purple","C":"blue","I":"gray"}
    L=["title: RACI matrix (faked)",""]
    for j,p in enumerate(people):
        L.append(f"h{j}: {p} [text, x: {260+j*120}, y: 20, w: 110]")
    for i,(a,codes) in enumerate(acts):
        y=60+i*48
        L.append(f"a{i}: {a} [text, x: 20, y: {y+6}, w: 220]")
        for j,ch in enumerate(codes):
            L.append(f"c{i}{j}: {ch} [rounded, {colr[ch]}, x: {260+j*120+35}, y: {y}, w: 40, h: 36]")
    y=60+len(acts)*48+16
    for k,(ch,n) in enumerate([("R","Responsible"),("A","Accountable"),("C","Consulted"),("I","Informed")]):
        L.append(f"lg{k}: {ch} = {n} [rounded, {colr[ch]}, x: {20+k*160}, y: {y}, w: 150, h: 32]")
    return "\n".join(L)

open("03-gantt.diagram","w").write(gantt()+"\n")
open("05-risk-matrix.diagram","w").write(risk()+"\n")
open("07-raci.diagram","w").write(raci()+"\n")
