'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';

const NavigationLinks = () => {
    const pathname = usePathname();

    return <div className='flex items-center gap-3'></div>;
};

export default NavigationLinks;
