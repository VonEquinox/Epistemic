import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import {
  ArrowLeft,
  ArrowRight,
  CircleUserRound,
  FileText,
  Maximize2,
  Network,
  Quote,
  ShieldCheck,
  Sparkles,
  UsersRound,
  WandSparkles,
} from 'lucide-react';
import { Link } from 'react-router-dom';
import BlurText from '../components/reactbits/BlurText';
import CountUp from '../components/reactbits/CountUp';
import GradientText from '../components/reactbits/GradientText';
import SpotlightCard from '../components/reactbits/SpotlightCard';
import StarBorder from '../components/reactbits/StarBorder';
import MathFormula from '../components/MathFormula';
import './report.css';

type CardProps = { title: string; children: ReactNode; color?: string; icon?: ReactNode };

function Card({ title, children, color = '#7aa8ff', icon }: CardProps) {
  return (
    <SpotlightCard className="r-card" spotlightColor="rgba(122, 168, 255, 0.2)">
      <div className="r-card-mark" style={{ background: color, boxShadow: `0 0 18px ${color}` }}>
        {icon}
      </div>
      <h3>{title}</h3>
      <div className="r-card-copy">{children}</div>
    </SpotlightCard>
  );
}

const slides = [
  {
    kicker: '01 · THESIS',
    title: '让人和人共享知识，再让 AI 沿着这张知识工作。',
    body: 'Epistemic 的核心不是“再做一个论文搜索”，而是把团队的阅读、判断与证据组织成 AI 能真正理解的研究上下文。',
    content: (
      <div className="thesis-layout">
        <div className="thesis-flow">
          <div className="thesis-step human"><UsersRound size={22} /><b>人 ↔ 人</b><small>评论 · 争议 · 共识</small></div>
          <div className="thesis-arrow">→</div>
          <div className="thesis-step graph"><Network size={22} /><b>共享知识图</b><small>节点 · 证据 · 关系</small></div>
          <div className="thesis-arrow">→</div>
          <div className="thesis-step ai"><WandSparkles size={22} /><b>人 ↔ AI</b><small>上下文 · 推演 · 新想法</small></div>
        </div>
        <div className="thesis-note">研究组的每个判断，都会成为下一次提问的起点。</div>
      </div>
    ),
  },
  {
    kicker: '02 · WHY NOW',
    title: '个人知道很多，但团队和 AI 都看不见这些知识。',
    body: '问题不在于缺少信息，而在于信息没有被共享、定位和复用。',
    content: (
      <div className="cards">
        <Card title="知识停在个人脑中" icon={<CircleUserRound size={16} />}><p>“我看过、我怀疑、我觉得相关”无法自然进入团队记忆。</p></Card>
        <Card title="协作只留下结论" color="#a78bfa" icon={<UsersRound size={16} />}><p>群聊里的判断没有节点、证据和来源，下一位成员无法复盘。</p></Card>
        <Card title="AI 只能重新搜索" color="#67e8f9" icon={<WandSparkles size={16} />}><p>没有团队脉络，AI 只能从公开文本猜，不知道你们已经讨论过什么。</p></Card>
      </div>
    ),
  },
  {
    kicker: '03 · COLLABORATION LOOP',
    title: '人与人的协作，先把“看法”变成可组合的知识。',
    body: '每个人保留自己的声音；观点不覆盖，证据可追溯，冲突也可以被保留下来。',
    content: (
      <div className="loop-layout">
        <div className="loop-steps">
          <div className="loop-step"><span>01</span><b>读到一个节点</b><small>论文、Claim 或关系</small></div>
          <div className="loop-line" />
          <div className="loop-step"><span>02</span><b>留下自己的 comment</b><small>idea · thinking · review</small></div>
          <div className="loop-line" />
          <div className="loop-step"><span>03</span><b>形成团队上下文</b><small>按用户、节点、图组织</small></div>
        </div>
        <div className="comment-stack">
          <div className="comment-chip"><b>余子豪 · idea</b><span>这里可以接入方法谱系。</span></div>
          <div className="comment-chip violet"><b>张宇翱 · review</b><span>原文第 5 页支持这个判断。</span></div>
          <div className="comment-chip cyan"><b>赖梦琪 · thinking</b><span>可能是互补，而不是替代。</span></div>
        </div>
      </div>
    ),
  },
  {
    kicker: '04 · DATA MODEL',
    title: '共享的不是一张漂亮的图，而是一组可回到原文的判断。',
    body: '节点保存论文；关系保存语义；comment 保存人的视角；证据保存为什么。',
    content: (
      <div className="data-model-wrap"><div className="data-model">
        <div className="data-node paper-node"><FileText size={19} /><b>Paper / Node</b><small>title · year · source</small></div>
        <div className="data-connector c1">has</div>
        <div className="data-node evidence-node"><Quote size={19} /><b>Evidence</b><small>span · page · bbox</small></div>
        <div className="data-connector c2">receives</div>
        <div className="data-node comment-node"><CircleUserRound size={19} /><b>User Comment</b><small>user · type · content</small></div>
        <div className="data-connector c3">supports</div>
        <div className="data-node relation-node"><Network size={19} /><b>Relation</b><small>type · confidence · review</small></div>
      </div>
      <div className="technical-equations"><MathFormula tex={String.raw`r=(u,v,\tau,e,c,\rho)`} /><MathFormula tex={String.raw`m=(a,v,\kappa,text,t)`} /><span>author-scoped · graph-scoped · evidence-bound</span></div></div>
    ),
  },
  {
    kicker: '05 · LAYOUT ENGINE',
    title: '图的美观不是装饰：它把“谁和谁更接近”变成可读的空间。',
    body: 'fCoSE 先生成稳定基线，再用加权引力与全局斥力做 relaxation；参数可以现场调节。',
    content: (
      <div className="layout-showcase">
        <div className="force-visual">
          <div className="force-grid" />
          <svg className="force-svg" viewBox="0 0 500 280" role="img" aria-label="加权弹簧节点布局示意图">
            <line className="spring spring-strong" x1="90" y1="160" x2="225" y2="145" />
            <line className="spring spring-medium" x1="225" y1="145" x2="355" y2="70" />
            <line className="spring spring-weak" x1="225" y1="145" x2="420" y2="210" />
            <g className="svg-node svg-node-a"><circle cx="90" cy="160" r="25" /><text x="90" y="166">A</text></g>
            <g className="svg-node svg-node-b"><circle cx="225" cy="145" r="25" /><text x="225" y="151">B</text></g>
            <g className="svg-node svg-node-c"><circle cx="355" cy="70" r="25" /><text x="355" y="76">C</text></g>
            <g className="svg-node svg-node-d"><circle cx="420" cy="210" r="25" /><text x="420" y="216">D</text></g>
          </svg>
          <div className="force-legend attraction-legend"><i />强关系：拉近</div>
          <div className="force-legend repulsion-legend"><i />全局：互相避让</div>
        </div>
        <div className="layout-side">
          <div className="formula-card"><MathFormula tex={String.raw`\mathbf F_a=k_a\hat w_{ij}^{\alpha}(d_{ij}-L_{ij})\hat{\mathbf d}_{ij}`} display /><span>加权弹簧：高相关边保持强拉力</span></div>
          <div className="formula-card"><MathFormula tex={String.raw`\mathbf F_r=\frac{k_r}{(d_{ij}/d_0)^{\beta}}\hat{\mathbf d}_{ji}`} display /><span>全局斥力：近距离强，远距离快速衰减</span></div>
          <div className="target-length"><MathFormula tex={String.raw`L_{ij}=L_{min}+(L_{max}-L_{min})(1-s_{ij})^{\gamma}`} /><span>score → distance</span></div>
          <div className="tuning-row"><span>live tuning</span><b>引力 2.0</b><b>斥力 0.5</b><b>衰减 2 / 1</b></div>
        </div>
      </div>
    ),
  },
  {
    kicker: '06 · HUMAN → AI',
    title: '当团队把知识共享出来，AI 才能真正“接着你们想”。',
    body: 'MCP 不只是查论文；它把图、邻居、原文和每个人的 comment 组装成一次可用的上下文。',
    content: (
      <div className="ai-loop">
        <div className="ai-loop-row"><div className="ai-pill human-pill"><UsersRound size={18} />团队观点</div><span>→</span><div className="ai-pill graph-pill"><Network size={18} />结构化图</div><span>→</span><div className="ai-pill mcp-pill"><WandSparkles size={18} />MCP context</div><span>→</span><div className="ai-pill idea-pill"><Sparkles size={18} />更贴合的 idea</div></div>
        <div className="mcp-console"><p><em>$</em> epistemic.get_context(node_id)</p><p><em>→</em> graph + neighbors + source + comments</p><MathFormula tex={String.raw`C(v,k)=G_k(v)\cup E(v)\cup M(v)\cup R(v)`} display className="console-math" /><strong>AI receives the team’s trail, not just the web’s index.</strong></div>
      </div>
    ),
  },
  {
    kicker: '07 · MCP SURFACE',
    title: '暴露的是研究脉络，而不是一堆孤立字段。',
    body: '模型可以沿着“节点 → 邻居 → 原文 → comment → 关系”逐层深入，再回到用户的问题。',
    content: (
      <div className="mcp-grid"><div className="mcp-tree"><div className="tree-root">get_graph()</div><div className="tree-branch">├─ get_node(id)</div><div className="tree-branch">│  ├─ get_source()</div><div className="tree-branch">│  └─ get_comments()</div><div className="tree-branch">└─ get_neighbors(id)</div><div className="tree-branch">   └─ get_relations()</div><MathFormula tex={String.raw`x^*=\arg\max_{x\in N_k(v)}[\lambda s(x,v)+(1-\lambda)q(x)]`} display className="tree-math" /></div><div className="mcp-result"><div><span>context assembled</span><b>12 nodes · 26 relations</b></div><div><span>human signals</span><b>41 comments · 3 reviewers</b></div><div><span>output</span><b>an idea with a trail</b></div></div></div>
    ),
  },
  {
    kicker: '08 · TRUST BOUNDARY',
    title: 'AI 负责放大，团队负责确认；系统负责留下证据。',
    body: '把人和 AI 的边界写进数据和流程，才不会把猜测误当成共识。',
    content: (
      <div className="boundary-wrap"><div className="boundary-row"><Card title="人：提出与修正" icon={<UsersRound size={16} />}><p>comment、关系、review 都保留作者和时间。</p></Card><Card title="AI：发现与连接" color="#67e8f9" icon={<WandSparkles size={16} />}><p>只读获取上下文，生成候选与推演，不覆盖人的判断。</p></Card><Card title="系统：约束与追溯" color="#fbbf24" icon={<ShieldCheck size={16} />}><p>无证据不入图，高风险关系确认前不上主图。</p></Card></div><div className="state-pipeline"><span>candidate</span><i>→</i><span>confirmed</span><i>/</i><span>disputed</span><i>/</i><span>rejected</span><b>append-only review trail</b></div></div>
    ),
  },
  {
    kicker: '09 · TAKEAWAY',
    title: '共享知识，让人类协作有记忆，让 AI 协作有方向。',
    body: '这就是 Epistemic 的价值：把团队已经想过的，变成 AI 下一次可以继续想的。',
    content: (
      <div className="end"><Sparkles color="#9ec5ff" size={31} /><GradientText colors={['#9ec5ff', '#d8c4ff', '#6fe7ff']} className="end-title">Human knowledge → better AI work</GradientText><div className="end-stats"><div><CountUp to={1} /><span>张共享图</span></div><div><CountUp to={0} /><span>观点不覆盖</span></div><div><CountUp to={100} /><span>上下文可追溯</span></div></div><div className="end-actions"><StarBorder as="a" href={import.meta.env.VITE_APP_URL ?? '/'} color="#7aa8ff"><span>进入系统</span></StarBorder><Link to="/">回到官网 <ArrowRight size={15} /></Link></div></div>
    ),
  },
];

export default function ReportPage() {
  const [index, setIndex] = useState(0);
  const [full, setFull] = useState(false);
  const go = useCallback((delta: number) => setIndex((n) => Math.max(0, Math.min(slides.length - 1, n + delta))), []);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (['ArrowRight', 'ArrowDown', ' ', 'PageDown'].includes(e.key)) { e.preventDefault(); go(1); }
      else if (['ArrowLeft', 'ArrowUp', 'PageUp'].includes(e.key)) { e.preventDefault(); go(-1); }
      else if (e.key === 'Home') setIndex(0);
      else if (e.key === 'End') setIndex(slides.length - 1);
    };
    addEventListener('keydown', onKey); return () => removeEventListener('keydown', onKey);
  }, [go]);
  const progress = useMemo(() => ((index + 1) / slides.length) * 100, [index]);
  const toggleFullscreen = async () => { const root = document.querySelector('.report-deck'); if (!document.fullscreenElement) { await root?.requestFullscreen?.(); setFull(true); } else { await document.exitFullscreen?.(); setFull(false); } };
  const slide = slides[index];
  return (
    <main className="report-deck" onWheel={(e) => { if (Math.abs(e.deltaY) > 24) go(e.deltaY > 0 ? 1 : -1); }}>
      <div className="r-progress" style={{ width: `${progress}%` }} />
      <header className="r-head"><Link to="/"><ArrowLeft size={15} /> 官网</Link><div className="dots">{slides.map((_, i) => <button key={i} aria-label={`第 ${i + 1} 页`} className={i === index ? 'active' : ''} onClick={() => setIndex(i)} />)}</div><button onClick={toggleFullscreen}><Maximize2 size={15} />{full ? '退出' : '全屏'}</button></header>
      <section className={`r-slide r-slide-${index + 1}`} key={index}>
        <div className="r-meta"><span>{slide.kicker}</span><span>TECHNICAL DEFENSE · 2026</span></div>
        <BlurText text={slide.title} delay={22} className="r-title" />
        <p className="r-body">{slide.body}</p>
        <div className="r-content">{slide.content}</div>
        <footer><button disabled={!index} onClick={() => go(-1)}><ArrowLeft size={15} />上一页</button><button disabled={index === slides.length - 1} onClick={() => go(1)}>下一页<ArrowRight size={15} /></button></footer>
      </section>
    </main>
  );
}
