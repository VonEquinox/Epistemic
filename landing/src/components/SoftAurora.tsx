import type { CSSProperties } from 'react';
type Props={color1?:string;color2?:string;speed?:number;brightness?:number;scale?:number;enableMouseInteraction?:boolean;mouseInfluence?:number};
export default function SoftAurora({color1='#0b57d0',color2='#715573',speed=.5,brightness=.7,scale=1}:Props){const style={'--aurora-a':color1,'--aurora-b':color2,'--aurora-speed':`${Math.max(8,24/Math.max(.1,speed))}s`,'--aurora-opacity':brightness,transform:`scale(${scale})`} as CSSProperties;return <div className="soft-aurora-fallback" style={style}/>}
