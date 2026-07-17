import { motion } from 'motion/react';
import type { ReactNode } from 'react';
export default function FadeContent({children,className='',delay=0,duration=600}:{children:ReactNode;className?:string;delay?:number;duration?:number}){return <motion.div className={className} initial={{opacity:0,y:16}} whileInView={{opacity:1,y:0}} viewport={{once:true,amount:.15}} transition={{duration:duration/1000,delay:delay/1000}}>{children}</motion.div>}
