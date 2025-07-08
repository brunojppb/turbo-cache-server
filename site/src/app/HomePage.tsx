import Image from 'next/image';

import ExtensionDetails from '@/app/ExtensionDetails';
import SetupDetails from '@/app/SetupDetails';

const HomePage: React.FC = () => {
    return (
        <div className='bg-background relative min-h-screen overflow-hidden'>
            {/* Background Pattern */}
            <div className='bg-grid-pattern absolute inset-0 opacity-[0.03] dark:opacity-[0.05]' />
            <div className='absolute inset-0 bg-gradient-to-br from-blue-50/50 via-transparent to-purple-50/50 dark:from-blue-950/20 dark:via-transparent dark:to-purple-950/20' />

            {/* Hero Section */}
            <main className='relative'>
                <div className='mx-auto max-w-7xl px-4 sm:px-6 lg:px-8'>
                    <div className='pt-20 pb-16 text-center lg:pt-32'>
                        {/* Main Heading */}
                        <h1 className='font-display mx-auto max-w-4xl text-5xl font-medium tracking-tight text-slate-900 sm:text-7xl dark:text-white'>
                            Make{' '}
                            <span className='relative whitespace-nowrap text-blue-600 dark:text-blue-400'>
                                <svg
                                    aria-hidden='true'
                                    viewBox='0 0 418 42'
                                    className='absolute top-2/3 left-0 h-[0.58em] w-full fill-blue-300/70 dark:fill-blue-500/40'
                                    preserveAspectRatio='none'>
                                    <path d='m203.371.916c-26.013-2.078-76.686 1.963-124.73 9.946L67.3 12.749C35.421 18.062 18.2 21.766 6.004 25.934 1.244 27.561.828 27.778.874 28.61c.07 1.214.828 1.121 9.595-1.176 9.072-2.377 17.15-3.92 39.246-7.496C123.565 7.986 157.869 4.492 195.942 5.046c7.461.108 19.25 1.696 19.17 2.582-.107 1.183-7.874 4.31-25.75 10.366-21.992 7.45-35.43 12.534-36.701 13.884-2.173 2.308-.202 4.407 4.442 4.734 2.654.187 3.263.157 15.593-.78 35.401-2.686 57.944-3.488 88.365-3.143 46.327.526 75.721 2.23 130.788 7.584 19.787 1.924 20.814 1.98 24.557 1.332l.066-.011c1.201-.203 1.53-1.825.399-2.335-2.911-1.31-4.893-1.604-22.048-3.261-57.509-5.556-87.871-7.36-132.059-7.842-23.239-.254-33.617-.116-50.627.674-11.629.54-42.371 2.494-46.696 2.967-2.359.259 8.133-3.625 26.504-9.81 23.239-7.825 27.934-10.149 28.304-14.005.417-4.348-3.529-6-16.878-7.066Z' />
                                </svg>
                                <span className='relative'>ship</span>
                            </span>{' '}
                            happen
                        </h1>

                        {/* Subtitle */}
                        <p className='mx-auto mt-6 max-w-2xl text-lg tracking-tight text-slate-700 dark:text-slate-300'>
                            Turborepo remote cache server, API-compliant as a GitHub Action or Docker with S3-compatible
                            storage support.
                        </p>

                        {/* Stats */}
                        <div className='mt-10 flex justify-center'>
                            <div className='flex items-center space-x-2 text-sm text-slate-600 dark:text-slate-400'>
                                <div className='flex items-center space-x-1'>
                                    <div className='h-2 w-2 rounded-full bg-green-500'></div>
                                    <span className='font-medium'>2,934,507</span>
                                </div>
                                <span>hours of compute saved</span>
                            </div>
                        </div>

                        {/* CTA Buttons */}
                        <div className='mt-10 flex justify-center gap-x-6'>
                            <a
                                className='group inline-flex items-center justify-center rounded-full bg-slate-900 px-4 py-2 text-sm font-semibold text-white hover:bg-slate-700 hover:text-slate-100 focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 active:bg-slate-800 active:text-slate-300 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200 dark:focus-visible:outline-slate-100 dark:active:bg-slate-200'
                                href='https://github.com/brunojppb/turbo-cache-server'
                                target='_blank'
                                rel='noopener noreferrer'>
                                Get started
                            </a>
                            <div className='flex items-center space-x-2 text-sm text-slate-600 dark:text-slate-400'>
                                <span>$</span>
                                <code className='rounded bg-slate-100 px-2 py-1 text-slate-800 dark:bg-slate-800 dark:text-slate-200'>
                                    docker run ghcr.io/brunojppb/turbo-cache-server
                                </code>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Feature Sections */}
                <div className='mx-auto max-w-7xl px-4 sm:px-6 lg:px-8'>
                    <div className='py-24'>
                        <div className='mx-auto max-w-2xl lg:mx-0'>
                            <h2 className='text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white'>
                                Scale your workflows
                            </h2>
                            <p className='mt-6 text-lg leading-8 text-slate-600 dark:text-slate-300'>
                                Optimize your CI tasks and save engineering time with remote caching for Turborepo.
                            </p>
                        </div>
                        <div className='mx-auto mt-16 max-w-2xl sm:mt-20 lg:mt-24 lg:max-w-none'>
                            <dl className='grid max-w-xl grid-cols-1 gap-x-8 gap-y-16 lg:max-w-none lg:grid-cols-3'>
                                <div className='flex flex-col'>
                                    <dt className='text-base leading-7 font-semibold text-slate-900 dark:text-white'>
                                        <div className='mb-6 flex h-10 w-10 items-center justify-center rounded-lg bg-blue-600'>
                                            <svg
                                                className='h-6 w-6 text-white'
                                                fill='none'
                                                viewBox='0 0 24 24'
                                                strokeWidth='1.5'
                                                stroke='currentColor'>
                                                <path
                                                    strokeLinecap='round'
                                                    strokeLinejoin='round'
                                                    d='M2.25 15a4.5 4.5 0 004.5 4.5H18a3.75 3.75 0 001.332-7.257 3 3 0 00-3.758-3.848 5.25 5.25 0 00-10.233 2.33A4.502 4.502 0 002.25 15z'
                                                />
                                            </svg>
                                        </div>
                                        Works with any provider
                                    </dt>
                                    <dd className='mt-1 flex flex-auto flex-col text-base leading-7 text-slate-600 dark:text-slate-300'>
                                        <p className='flex-auto'>
                                            Integrate with GitHub Actions, GitLab CI, or any CI provider for speed at
                                            all scales
                                        </p>
                                    </dd>
                                </div>
                                <div className='flex flex-col'>
                                    <dt className='text-base leading-7 font-semibold text-slate-900 dark:text-white'>
                                        <div className='mb-6 flex h-10 w-10 items-center justify-center rounded-lg bg-blue-600'>
                                            <svg
                                                className='h-6 w-6 text-white'
                                                fill='none'
                                                viewBox='0 0 24 24'
                                                strokeWidth='1.5'
                                                stroke='currentColor'>
                                                <path
                                                    strokeLinecap='round'
                                                    strokeLinejoin='round'
                                                    d='M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 0v3.75m-16.5-3.75v3.75m16.5 0v3.75C20.25 16.153 16.556 18 12 18s-8.25-1.847-8.25-4.125v-3.75m16.5 0c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125'
                                                />
                                            </svg>
                                        </div>
                                        Remote Caching
                                    </dt>
                                    <dd className='mt-1 flex flex-auto flex-col text-base leading-7 text-slate-600 dark:text-slate-300'>
                                        <p className='flex-auto'>
                                            Never do the same work twice with S3-compatible storage
                                        </p>
                                    </dd>
                                </div>
                                <div className='flex flex-col'>
                                    <dt className='text-base leading-7 font-semibold text-slate-900 dark:text-white'>
                                        <div className='mb-6 flex h-10 w-10 items-center justify-center rounded-lg bg-blue-600'>
                                            <svg
                                                className='h-6 w-6 text-white'
                                                fill='none'
                                                viewBox='0 0 24 24'
                                                strokeWidth='1.5'
                                                stroke='currentColor'>
                                                <path
                                                    strokeLinecap='round'
                                                    strokeLinejoin='round'
                                                    d='M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z'
                                                />
                                                <path
                                                    strokeLinecap='round'
                                                    strokeLinejoin='round'
                                                    d='M15 12a3 3 0 11-6 0 3 3 0 016 0z'
                                                />
                                            </svg>
                                        </div>
                                        Effortless setup
                                    </dt>
                                    <dd className='mt-1 flex flex-auto flex-col text-base leading-7 text-slate-600 dark:text-slate-300'>
                                        <p className='flex-auto'>
                                            Deploy as a GitHub Action or Docker container in minutes
                                        </p>
                                    </dd>
                                </div>
                            </dl>
                        </div>
                    </div>
                </div>

                {/* Simple Setup Section */}
                <div className='bg-slate-50 dark:bg-slate-900/50'>
                    <div className='mx-auto max-w-7xl px-4 py-24 sm:px-6 lg:px-8'>
                        <div className='mx-auto max-w-2xl lg:mx-0'>
                            <h2 className='text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white'>
                                Simple setup
                            </h2>
                            <p className='mt-6 text-lg leading-8 text-slate-600 dark:text-slate-300'>
                                Start caching your Turborepo builds in minutes with GitHub Actions or Docker.
                            </p>
                        </div>
                        <div className='mt-16 overflow-hidden rounded-lg bg-white shadow-xl dark:bg-slate-800'>
                            <div className='px-6 py-8'>
                                <div className='flex items-center justify-between'>
                                    <h3 className='text-lg font-medium text-slate-900 dark:text-white'>
                                        GitHub Action
                                    </h3>
                                    <a
                                        href='https://github.com/brunojppb/turbo-cache-server#readme'
                                        className='text-sm text-blue-600 hover:text-blue-500 dark:text-blue-400'
                                        target='_blank'
                                        rel='noopener noreferrer'>
                                        Read the docs
                                    </a>
                                </div>
                                <div className='mt-4'>
                                    <pre className='overflow-x-auto rounded-md bg-slate-900 p-4 text-sm text-white dark:bg-slate-700'>
                                        <code>{`- name: Turborepo Cache Server
  uses: brunojppb/turbo-cache-server@1.0.24
  env:
    PORT: "8585"
    S3_BUCKET_NAME: your-bucket-name-here
    S3_REGION: "eu-central-1"
    S3_ACCESS_KEY: \${{ secrets.S3_ACCESS_KEY }}
    S3_SECRET_KEY: \${{ secrets.S3_SECRET_KEY }}`}</code>
                                    </pre>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </main>
        </div>
    );
};

export default HomePage;
