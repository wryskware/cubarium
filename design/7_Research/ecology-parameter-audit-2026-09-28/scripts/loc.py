import os, re, sys
ROOT=__import__("os").path.abspath(__import__("os").path.join(__import__("os").path.dirname(__file__), "../../../.."))
def classify(path):
    lines=open(path,encoding='utf-8',errors='replace').read().split('\n')
    n=len(lines); test=[False]*n
    i=0
    while i<n:
        if lines[i].strip().startswith('#[cfg(test)]'):
            # find the next item start; if it's `mod x {`, brace-match it
            j=i+1
            while j<n and lines[j].strip()=='' : j+=1
            if j<n and re.match(r'\s*(pub(\(crate\))?\s+)?mod\s+\w+\s*\{',lines[j]):
                depth=0; k=j; started=False
                while k<n:
                    s=re.sub(r'//.*','',lines[k])
                    depth+=s.count('{')-s.count('}')
                    if '{' in s: started=True
                    test[k]=True
                    if started and depth<=0: break
                    k+=1
                for m in range(i,k+1): test[m]=True
                i=k+1; continue
        i+=1
    c={'blank':0,'doc':0,'comment':0,'code':0,'test':0}
    for k,l in enumerate(lines):
        s=l.strip()
        if test[k]: c['test']+=1
        elif s=='': c['blank']+=1
        elif s.startswith('///') or s.startswith('//!'): c['doc']+=1
        elif s.startswith('//'): c['comment']+=1
        else: c['code']+=1
    return c
def walk(sub):
    out={}
    for dp,dn,fn in os.walk(os.path.join(ROOT,sub)):
        for f in fn:
            if f.endswith('.rs'):
                p=os.path.join(dp,f); rel=os.path.relpath(p,ROOT)
                kind='tests' if '/tests/' in rel or f.endswith('_tests.rs') else ('examples' if '/examples/' in rel else 'src')
                out.setdefault(kind,[]).append(p)
    return out
tot_all={}
for crate in sys.argv[1:]:
    groups=walk(crate)
    for kind in ('src','tests','examples'):
        files=groups.get(kind,[])
        if not files: continue
        c={'blank':0,'doc':0,'comment':0,'code':0,'test':0}
        for p in files:
            for k,v in classify(p).items(): c[k]+=v
        t=sum(c.values())
        print(f"{crate:42s} {kind:8s} total {t:6d} | logic {c['code']:6d} ({c['code']/t:4.0%}) doc {c['doc']:6d} ({c['doc']/t:4.0%}) //cmt {c['comment']:5d} ({c['comment']/t:4.0%}) in-file tests {c['test']:6d} ({c['test']/t:4.0%}) blank {c['blank']:5d}")
