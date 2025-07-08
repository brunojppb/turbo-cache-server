'use client';

import * as React from 'react';

import { useTheme } from 'next-themes';

import { Button } from '@/registry/new-york-v4/ui/button';

import { MoonIcon, SunIcon } from 'lucide-react';

export function ModeToggle() {
    const { setTheme, resolvedTheme } = useTheme();

    const toggleTheme = React.useCallback(() => {
        setTheme(resolvedTheme === 'dark' ? 'light' : 'dark');
    }, [resolvedTheme, setTheme]);

    return (
        <Button variant='ghost' className='group/toggle h-8 w-8 px-0' onClick={toggleTheme}>
            <SunIcon className='hidden [html.dark_&]:block' />
            <MoonIcon className='hidden [html.light_&]:block' />
            <span className='sr-only'>Toggle theme</span>
        </Button>
    );
}
