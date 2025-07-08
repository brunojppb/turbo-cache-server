import { type Metadata } from 'next';

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
                            Turborepo remote cache server that{' '}
                            <span className='relative whitespace-nowrap text-blue-600 dark:text-blue-400'>
                                <svg
                                    aria-hidden='true'
                                    viewBox='0 0 418 42'
                                    className='absolute top-2/3 left-0 h-[0.58em] w-full fill-blue-300/70 dark:fill-blue-500/40'
                                    preserveAspectRatio='none'>
                                    <path d='m203.371.916c-26.013-2.078-76.686 1.963-124.73 9.946L67.3 12.749C35.421 18.062 18.2 21.766 6.004 25.934 1.244 27.561.828 27.778.874 28.61c.07 1.214.828 1.121 9.595-1.176 9.072-2.377 17.15-3.92 39.246-7.496C123.565 7.986 157.869 4.492 195.942 5.046c7.461.108 19.25 1.696 19.17 2.582-.107 1.183-7.874 4.31-25.75 10.366-21.992 7.45-35.43 12.534-36.701 13.884-2.173 2.308-.202 4.407 4.442 4.734 2.654.187 3.263.157 15.593-.78 35.401-2.686 57.944-3.488 88.365-3.143 46.327.526 75.721 2.23 130.788 7.584 19.787 1.924 20.814 1.98 24.557 1.332l.066-.011c1.201-.203 1.53-1.825.399-2.335-2.911-1.31-4.893-1.604-22.048-3.261-57.509-5.556-87.871-7.36-132.059-7.842-23.239-.254-33.617-.116-50.627.674-11.629.54-42.371 2.494-46.696 2.967-2.359.259 8.133-3.625 26.504-9.81 23.239-7.825 27.934-10.149 28.304-14.005.417-4.348-3.529-6-16.878-7.066Z' />
                                </svg>
                                <span className='relative'>just works</span>
                            </span>
                        </h1>

                        {/* Subtitle */}
                        <p className='mx-auto mt-6 max-w-2xl text-lg tracking-tight text-slate-700 dark:text-slate-300'>
                            A tiny web server written in{' '}
                            <a href='https://www.rust-lang.org/'>
                                <span className='text-yellow-500 hover:underline dark:text-yellow-600'>Rust</span>
                            </a>
                            {', '}
                            providing distributed remote caching for{' '}
                            <a
                                href='https://turborepo.com/'
                                className='text-pink-600 hover:underline'
                                rel='noopener noreferrer'>
                                Turborepo
                            </a>
                            . Deploy as a GitHub Action or Docker container with S3-compatible storage.
                        </p>

                        {/* Storage Providers */}
                        <div className='mt-10 flex justify-center'>
                            <div className='flex flex-col items-center space-y-2'>
                                <p className='text-sm text-slate-600 dark:text-slate-400'>
                                    Tested with popular storage providers:
                                </p>
                                <div className='flex items-center space-x-6 text-sm text-slate-500 dark:text-slate-400'>
                                    <span className='font-medium'>Amazon S3</span>
                                    <span>•</span>
                                    <span className='font-medium'>Cloudflare R2</span>
                                    <span>•</span>
                                    <span className='font-medium'>MinIO</span>
                                </div>
                            </div>
                        </div>

                        {/* CTA Buttons */}
                        <div className='mt-10 flex flex-col items-center gap-4 sm:justify-center'>
                            <a
                                className='group inline-flex items-center justify-center rounded-full bg-slate-900 px-6 py-3 text-sm font-semibold text-white hover:bg-slate-700 hover:text-slate-100 focus:outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-slate-900 active:bg-slate-800 active:text-slate-300 dark:bg-slate-100 dark:text-slate-900 dark:hover:bg-slate-200 dark:focus-visible:outline-slate-100 dark:active:bg-slate-200'
                                href='https://github.com/brunojppb/turbo-cache-server'
                                target='_blank'
                                rel='noopener noreferrer'>
                                <svg className='mr-2 h-4 w-4' fill='currentColor' viewBox='0 0 20 20'>
                                    <path
                                        fillRule='evenodd'
                                        d='M10 0C4.477 0 0 4.484 0 10.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0110 4.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.203 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.942.359.31.678.921.678 1.856 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0020 10.017C20 4.484 15.522 0 10 0z'
                                        clipRule='evenodd'
                                    />
                                </svg>
                                Star it on GitHub
                            </a>
                        </div>
                    </div>
                </div>

                {/* Feature Sections */}
                <div className='mx-auto max-w-7xl px-4 sm:px-6 lg:px-8'>
                    <div className='mx-auto text-center lg:mx-0'>
                        <h2 className='text-center text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white'>
                            Why choose Turbo Cache Server?
                        </h2>
                        <p className='mt-6 text-lg leading-8 text-slate-600 dark:text-slate-300'>
                            Built with performance and simplicity in mind. Never rebuild the same artifacts twice.
                        </p>
                    </div>
                    <div className='mx-auto mt-16 max-w-2xl sm:mt-20 lg:mt-24 lg:max-w-none'>
                        <dl className='grid max-w-xl grid-cols-1 gap-x-8 gap-y-16 lg:max-w-none lg:grid-cols-3'>
                            <div className='flex flex-col'>
                                <dt className='text-base leading-7 font-semibold text-slate-900 dark:text-white'>
                                    <div className='mb-6 flex h-10 w-10 items-center justify-center rounded-lg bg-orange-600'>
                                        <svg
                                            className='h-6 w-6 text-white'
                                            fill='none'
                                            viewBox='0 0 24 24'
                                            strokeWidth='1.5'
                                            stroke='currentColor'>
                                            <path
                                                strokeLinecap='round'
                                                strokeLinejoin='round'
                                                d='M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z'
                                            />
                                        </svg>
                                    </div>
                                    Built with Rust
                                </dt>
                                <dd className='mt-1 flex flex-auto flex-col text-base leading-7 text-slate-600 dark:text-slate-300'>
                                    <p className='flex-auto'>
                                        Tiny, fast web server written in Rust for maximum performance and reliability
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
                                                d='M2.25 15a4.5 4.5 0 004.5 4.5H18a3.75 3.75 0 001.332-7.257 3 3 0 00-3.758-3.848 5.25 5.25 0 00-10.233 2.33A4.502 4.502 0 002.25 15z'
                                            />
                                        </svg>
                                    </div>
                                    S3-Compatible Storage
                                </dt>
                                <dd className='mt-1 flex flex-auto flex-col text-base leading-7 text-slate-600 dark:text-slate-300'>
                                    <p className='flex-auto'>
                                        Works with Amazon S3, Cloudflare R2, MinIO, and any S3-compatible storage
                                        provider
                                    </p>
                                </dd>
                            </div>
                            <div className='flex flex-col'>
                                <dt className='text-base leading-7 font-semibold text-slate-900 dark:text-white'>
                                    <div className='mb-6 flex h-10 w-10 items-center justify-center rounded-lg bg-green-600'>
                                        <svg
                                            className='h-6 w-6 text-white'
                                            fill='none'
                                            viewBox='0 0 24 24'
                                            strokeWidth='1.5'
                                            stroke='currentColor'>
                                            <path
                                                strokeLinecap='round'
                                                strokeLinejoin='round'
                                                d='M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z'
                                            />
                                        </svg>
                                    </div>
                                    API-Compliant
                                </dt>
                                <dd className='mt-1 flex flex-auto flex-col text-base leading-7 text-slate-600 dark:text-slate-300'>
                                    <p className='flex-auto'>
                                        Fully compatible with Turborepo's remote caching API - drop-in replacement
                                    </p>
                                </dd>
                            </div>
                        </dl>
                    </div>
                </div>

                {/* Companies Using Section */}
                <div className='mx-auto max-w-7xl px-4 py-24 sm:px-6 lg:px-8'>
                    <div className='mx-auto text-center lg:mx-0'>
                        <h2 className='text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white'>
                            Folks from the following companies using it
                        </h2>
                    </div>
                    <div className='mx-auto mt-16 max-w-2xl sm:mt-20 lg:mt-24 lg:max-w-none'>
                        <div className='grid grid-cols-1 justify-items-center gap-8 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-5'>
                            <a
                                href='https://n26.com'
                                target='_blank'
                                rel='noopener noreferrer'
                                className='group flex items-center justify-center rounded-2xl border border-slate-200 p-8 transition-colors hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-600 dark:hover:bg-slate-800/50'>
                                <div className='text-center'>
                                    <div className='text-2xl font-bold text-slate-900 group-hover:text-slate-700 dark:text-white dark:group-hover:text-slate-200'>
                                        N26
                                    </div>
                                    <div className='mt-2 text-sm text-slate-600 dark:text-slate-400'>
                                        Digital Banking
                                    </div>
                                </div>
                            </a>
                            <a
                                href='https://www.lego.com/en-us'
                                target='_blank'
                                rel='noopener noreferrer'
                                className='group flex items-center justify-center rounded-2xl border border-slate-200 p-8 transition-colors hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-600 dark:hover:bg-slate-800/50'>
                                <div className='text-center'>
                                    <div className='text-2xl font-bold text-slate-900 group-hover:text-slate-700 dark:text-white dark:group-hover:text-slate-200'>
                                        LEGO
                                    </div>
                                    <div className='mt-2 text-sm text-slate-600 dark:text-slate-400'>
                                        Creative Play Solutions
                                    </div>
                                </div>
                            </a>
                            <a
                                href='https://cursor.com/'
                                target='_blank'
                                rel='noopener noreferrer'
                                className='group flex items-center justify-center rounded-2xl border border-slate-200 p-8 transition-colors hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-600 dark:hover:bg-slate-800/50'>
                                <div className='text-center'>
                                    <div className='text-2xl font-bold text-slate-900 group-hover:text-slate-700 dark:text-white dark:group-hover:text-slate-200'>
                                        Cursor
                                    </div>
                                    <div className='mt-2 text-sm text-slate-600 dark:text-slate-400'>
                                        AI Code Editor
                                    </div>
                                </div>
                            </a>
                            <a
                                href='https://amplitude.com/'
                                target='_blank'
                                rel='noopener noreferrer'
                                className='group flex items-center justify-center rounded-2xl border border-slate-200 p-8 transition-colors hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-600 dark:hover:bg-slate-800/50'>
                                <div className='text-center'>
                                    <div className='text-2xl font-bold text-slate-900 group-hover:text-slate-700 dark:text-white dark:group-hover:text-slate-200'>
                                        Amplitude
                                    </div>
                                    <div className='mt-2 text-sm text-slate-600 dark:text-slate-400'>
                                        Product Analytics
                                    </div>
                                </div>
                            </a>
                            <a
                                href='https://www.bbc.com/'
                                target='_blank'
                                rel='noopener noreferrer'
                                className='group flex items-center justify-center rounded-2xl border border-slate-200 p-8 transition-colors hover:border-slate-300 hover:bg-slate-50 dark:border-slate-700 dark:hover:border-slate-600 dark:hover:bg-slate-800/50'>
                                <div className='text-center'>
                                    <div className='text-2xl font-bold text-slate-900 group-hover:text-slate-700 dark:text-white dark:group-hover:text-slate-200'>
                                        BBC
                                    </div>
                                    <div className='mt-2 text-sm text-slate-600 dark:text-slate-400'>
                                        Broadcasting & Media
                                    </div>
                                </div>
                            </a>
                        </div>
                    </div>
                </div>

                {/* Deployment Options Section */}
                <div className='mx-auto max-w-7xl px-4 sm:px-6 lg:px-8'>
                    <div className='mx-auto text-center lg:mx-0'>
                        <h2 className='text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white'>
                            Two ways to deploy
                        </h2>
                        <p className='mt-6 text-lg leading-8 text-slate-600 dark:text-slate-300'>
                            Choose the deployment method that fits your workflow. Both options provide the same
                            performance and features.
                        </p>
                    </div>
                    <div className='mx-auto mt-16 max-w-2xl sm:mt-20 lg:mt-24 lg:max-w-none'>
                        <div className='grid grid-cols-1 gap-8 lg:grid-cols-2'>
                            <div className='rounded-2xl border border-slate-200 p-8 dark:border-slate-700'>
                                <h3 className='text-lg font-semibold text-slate-900 dark:text-white'>GitHub Action</h3>
                                <p className='mt-4 text-sm text-slate-600 dark:text-slate-300'>
                                    Perfect for GitHub workflows. Starts automatically in the background during your CI
                                    runs.
                                </p>
                                <div className='mt-6'>
                                    <h4 className='text-sm font-medium text-slate-900 dark:text-white'>
                                        Key benefits:
                                    </h4>
                                    <ul className='mt-2 space-y-1 text-sm text-slate-600 dark:text-slate-300'>
                                        <li>• Zero configuration required</li>
                                        <li>• Automatic startup and teardown</li>
                                        <li>• Integrates seamlessly with existing workflows</li>
                                    </ul>
                                </div>
                            </div>
                            <div className='rounded-2xl border border-slate-200 p-8 dark:border-slate-700'>
                                <h3 className='text-lg font-semibold text-slate-900 dark:text-white'>
                                    Docker Container
                                </h3>
                                <p className='mt-4 text-sm text-slate-600 dark:text-slate-300'>
                                    Universal deployment option. Works with GitLab CI, Jenkins, or any CI system that
                                    supports Docker.
                                </p>
                                <div className='mt-6'>
                                    <h4 className='text-sm font-medium text-slate-900 dark:text-white'>
                                        Key benefits:
                                    </h4>
                                    <ul className='mt-2 space-y-1 text-sm text-slate-600 dark:text-slate-300'>
                                        <li>• Works with any CI provider</li>
                                        <li>• Self-hosted deployment options</li>
                                        <li>• Full control over infrastructure</li>
                                    </ul>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                {/* Setup Examples Section */}
                <div className='bg-slate-50 dark:bg-slate-900/50'>
                    <div className='mx-auto max-w-7xl px-4 py-24 sm:px-6 lg:px-8'>
                        <div className='mx-auto max-w-2xl lg:mx-0'>
                            <h2 className='text-3xl font-bold tracking-tight text-slate-900 sm:text-4xl dark:text-white'>
                                Quick setup examples
                            </h2>
                            <p className='mt-6 text-lg leading-8 text-slate-600 dark:text-slate-300'>
                                Get started in minutes with these copy-paste examples for your preferred deployment
                                method.
                            </p>
                        </div>

                        {/* GitHub Action Example */}
                        <div className='mt-16 overflow-hidden rounded-lg bg-white shadow-xl dark:bg-slate-800'>
                            <div className='px-6 py-8'>
                                <div className='flex items-center justify-between'>
                                    <h3 className='text-lg font-medium text-slate-900 dark:text-white'>
                                        GitHub Action Setup
                                    </h3>
                                </div>
                                <div className='mt-4'>
                                    <pre className='overflow-x-auto rounded-md bg-slate-900 p-4 text-sm text-white dark:bg-slate-700'>
                                        <code>{`env:
  TURBO_API: "http://127.0.0.1:8585"
  TURBO_TEAM: "NAME_OF_YOUR_REPO_HERE"
  TURBO_TOKEN: "turbo-token"

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout repository
        uses: actions/checkout@v4

      - name: Turborepo Cache Server
        uses: brunojppb/turbo-cache-server@1.0.24
        env:
          PORT: "8585"
          S3_BUCKET_NAME: your-bucket-name-here
          S3_REGION: "eu-central-1"
          S3_ACCESS_KEY: \${{ secrets.S3_ACCESS_KEY }}
          S3_SECRET_KEY: \${{ secrets.S3_SECRET_KEY }}
        
          # Optional: If not using AWS, provide endpoint like "https://minio" for your instance.
          S3_ENDPOINT: \${{ secrets.S3_ENDPOINT }}
          # Optional: If your S3-compatible store does not support requests
          # like https://bucket.hostname.domain/. Setting "S3_USE_PATH_STYLE"
          # to true configures the S3 client to make requests like
          # https://hostname.domain/bucket instead.
          # Defaults to "false"
          S3_USE_PATH_STYLE: false
          # Max payload size for each cache object sent by Turborepo
          # Defaults to 100 MB
          # Requests larger than that, will get "HTTP 413: Entity Too Large" errors
          MAX_PAYLOAD_SIZE_IN_MB: "100"
      
      - name: Run tasks
        run: turbo run test build typecheck`}</code>
                                    </pre>
                                </div>
                            </div>
                        </div>

                        {/* Docker Example */}
                        <div className='mt-8 overflow-hidden rounded-lg bg-white shadow-xl dark:bg-slate-800'>
                            <div className='px-6 py-8'>
                                <div className='flex items-center justify-between'>
                                    <h3 className='text-lg font-medium text-slate-900 dark:text-white'>Docker Setup</h3>
                                    <span className='text-sm text-slate-500 dark:text-slate-400'>
                                        Works with GitLab CI, Jenkins, etc.
                                    </span>
                                </div>
                                <div className='mt-4'>
                                    <pre className='overflow-x-auto rounded-md bg-slate-900 p-4 text-sm text-white dark:bg-slate-700'>
                                        <code>{`docker run \\
  -e S3_ACCESS_KEY=KEY \\
  -e S3_SECRET_KEY=SECRET \\
  -e S3_BUCKET_NAME=my_cache_bucket \\
  -e S3_ENDPOINT=https://s3_endpoint_here \\
  -e S3_REGION=eu \\
  -p "8000:8000" \\
  ghcr.io/brunojppb/turbo-cache-server`}</code>
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
