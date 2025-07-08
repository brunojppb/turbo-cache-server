'use client';

import { Button } from '@/registry/new-york-v4/ui/button';

import { toast } from 'sonner';

export function SonnerDemo() {
    return (
        <Button
            variant='outline'
            onClick={() =>
                toast('Event has been created', {
                    description: 'Sunday, December 03, 2023 at 9:00 AM',
                    action: {
                        label: 'Undo',
                        onClick: () => console.log('Undo')
                    }
                })
            }>
            Show Toast
        </Button>
    );
}
