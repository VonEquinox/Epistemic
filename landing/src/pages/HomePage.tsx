import { Link } from 'react-router-dom';
import BlurText from '../components/BlurText';
import FadeContent from '../components/FadeContent';
import GradientText from '../components/GradientText';
import Magnet from '../components/Magnet';
import SoftAurora from '../components/SoftAurora';
import SpotlightCard from '../components/SpotlightCard';
import { Glass, Pill } from '../ui/Glass';

const APP_URL = import.meta.env.VITE_APP_URL ?? 'http://localhost:5173';
const GITHUB_URL =
  import.meta.env.VITE_GITHUB_URL ?? 'https://github.com/VonEquinox/Epistemic';

const FEATURES = [
  {
    title: '无证据不入图',
    body: '12 种白名单关系，绑定原文 span。给不出证据的候选直接丢弃。',
  },
  {
    title: '八层 DNA',
    body: '问题到定位的固定分析层 + 分面 embedding。相似度只排位，不画边。',
  },
  {
    title: '审核队列',
    body: 'AI 提议，人一键裁决。高风险关系确认前不上图。',
  },
  {
    title: '集体记忆',
    body: '阅读状态、Claim 判断、批注与图节点评论，按图隔离。',
  },
  {
    title: '三层分级',
    body: '公开事实 / 团队记录 / AI 候选，字段与视觉永不混淆。',
  },
  {
    title: '只读 MCP',
    body: '个人 token，把图与原文上下文交给编码助手。',
  },
];

export default function HomePage() {
  return (
    <div className="min-h-screen bg-[var(--bg)] text-[var(--text)]">
      <header className="sticky top-0 z-50 border-b border-white/10 bg-[#070b14]/55 backdrop-blur-2xl">
        <div className="landing-container flex h-14 items-center justify-between gap-3">
          <a href="#top" className="flex items-center gap-2.5 font-medium tracking-tight">
            <span className="inline-flex h-8 w-8 items-center justify-center rounded-xl border border-white/20 bg-white/10 text-xs font-bold text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.25)] backdrop-blur">
              E
            </span>
            Epistemic
          </a>
          <nav className="hidden items-center gap-1 md:flex">
            <a href="#features" className="rb-nav-link">
              能力
            </a>
            <a href="#principles" className="rb-nav-link">
              原则
            </a>
            <Link to="/report" className="rb-nav-link">
              项目汇报
            </Link>
          </nav>
          <div className="flex items-center gap-2">
            <a
              href={GITHUB_URL}
              target="_blank"
              rel="noreferrer"
              className="rb-nav-link hidden sm:inline-flex"
            >
              GitHub
            </a>
            <a href={APP_URL} className="rb-btn-primary !px-4 !py-2 text-xs sm:text-sm">
              进入应用
            </a>
          </div>
        </div>
      </header>

      <main>
        <section id="top" className="relative isolate overflow-hidden pb-24 pt-16 sm:pt-24">
          <div className="absolute inset-0 -z-20 opacity-80">
            <SoftAurora
              speed={0.55}
              scale={1.2}
              brightness={0.72}
              color1="#0b57d0"
              color2="#715573"
              enableMouseInteraction
              mouseInfluence={0.12}
            />
          </div>
          <div className="pointer-events-none absolute inset-0 -z-10 bg-gradient-to-b from-[#070b14]/25 via-[#070b14]/50 to-[#070b14]" />

          <div className="landing-container relative max-w-4xl">
            <FadeContent duration={650}>
              <Pill>
                <span className="h-1.5 w-1.5 rounded-full bg-[#7eb0ff] shadow-[0_0_10px_#7eb0ff]" />
                论文证据关系图 · 集体研究记忆
              </Pill>
            </FadeContent>

            <h1 className="mt-7 text-4xl font-bold leading-[1.08] tracking-tight sm:text-5xl lg:text-[3.6rem]">
              <BlurText text="每条边都带原文证据" delay={55} className="justify-start" />
              <div className="mt-2">
                <GradientText
                  colors={['#9ec5ff', '#ffffff', '#c4b5fd', '#7eb0ff']}
                  animationSpeed={9}
                  className="text-4xl font-bold sm:text-5xl lg:text-[3.6rem]"
                >
                  的论文关系图
                </GradientText>
              </div>
            </h1>

            <FadeContent delay={120} duration={650} className="mt-7 max-w-2xl">
              <Glass intense className="!rounded-[1.4rem]">
                <p className="p-5 text-base leading-relaxed text-[#d5def5] sm:text-lg sm:p-6">
                  把「论文之间到底什么关系」、组内阅读状态与 Claim 级判断，沉淀为可审核的团队记忆。
                  相似度只排位，断言才画边。
                </p>
              </Glass>
            </FadeContent>

            <FadeContent delay={200} duration={650} className="mt-8 flex flex-wrap gap-3">
              <Magnet padding={36} magnetStrength={2.5}>
                <a href={APP_URL} className="rb-btn-primary">
                  打开工作台
                </a>
              </Magnet>
              <Link to="/report" className="rb-btn-ghost">
                项目汇报
              </Link>
            </FadeContent>
          </div>
        </section>

        <section id="features" className="border-t border-white/10 py-20">
          <div className="landing-container">
            <FadeContent>
              <p className="rb-section-label">能力</p>
              <h2 className="mt-2 text-3xl font-bold tracking-tight sm:text-4xl">
                简洁，但每一步可追溯
              </h2>
            </FadeContent>

            <div className="mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {FEATURES.map((f, i) => (
                <FadeContent key={f.title} delay={i * 35}>
                  <SpotlightCard
                    className="h-full rounded-[1.25rem] border-0 bg-transparent p-0"
                    spotlightColor="rgba(126, 176, 255, 0.16)"
                  >
                    <Glass className="h-full !rounded-[1.25rem]">
                      <div className="p-5">
                        <h3 className="text-lg font-semibold text-white">{f.title}</h3>
                        <p className="mt-2 text-sm leading-relaxed text-[var(--muted)]">{f.body}</p>
                      </div>
                    </Glass>
                  </SpotlightCard>
                </FadeContent>
              ))}
            </div>
          </div>
        </section>

        <section id="principles" className="border-t border-white/10 py-20">
          <div className="landing-container">
            <FadeContent>
              <p className="rb-section-label">原则</p>
              <h2 className="mt-2 text-3xl font-bold tracking-tight sm:text-4xl">三条底线</h2>
            </FadeContent>
            <div className="mt-8 grid gap-3 sm:grid-cols-3">
              {['无证据不入图', 'AI 只提出候选', '相似度只决定位置'].map((p) => (
                <Glass key={p} className="!rounded-2xl">
                  <div className="px-5 py-4 text-sm font-medium text-[#e8eeff]">{p}</div>
                </Glass>
              ))}
            </div>
          </div>
        </section>

        <section className="pb-24 pt-6">
          <div className="landing-container">
            <Glass intense className="!rounded-[1.75rem]">
              <div className="relative overflow-hidden px-8 py-12 sm:px-12">
                <div
                  aria-hidden
                  className="pointer-events-none absolute -right-10 -top-16 h-48 w-48 rounded-full bg-[#7eb0ff]/20 blur-3xl"
                />
                <div className="relative max-w-2xl">
                  <h2 className="text-3xl font-bold tracking-tight sm:text-4xl">
                    给研究记忆一层结构
                  </h2>
                  <p className="mt-3 text-sm text-[var(--muted)] sm:text-base">
                    邀请制、组内 dogfood。完整答辩结构在独立汇报页。
                  </p>
                  <div className="mt-8 flex flex-wrap gap-3">
                    <a href={APP_URL} className="rb-btn-primary">
                      进入应用
                    </a>
                    <Link to="/report" className="rb-btn-ghost">
                      查看项目汇报
                    </Link>
                  </div>
                </div>
              </div>
            </Glass>
          </div>
        </section>
      </main>

      <footer className="border-t border-white/10 py-8">
        <div className="landing-container flex flex-col gap-3 text-sm text-[var(--muted)] sm:flex-row sm:items-center sm:justify-between">
          <div>
            <span className="font-medium text-white">Epistemic</span>
            <span className="mx-2 text-white/20">·</span>
            产品官网
          </div>
          <div className="flex flex-wrap gap-4">
            <Link to="/report" className="hover:text-white">
              项目汇报
            </Link>
            <a href={APP_URL} className="hover:text-white">
              应用
            </a>
            <a href={GITHUB_URL} target="_blank" rel="noreferrer" className="hover:text-white">
              GitHub
            </a>
          </div>
        </div>
      </footer>
    </div>
  );
}
