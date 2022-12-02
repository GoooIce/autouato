/** @type {import('next').NextConfig} */

const nextConfig = {
  reactStrictMode: true,
  swcMinify: true,
  // 注意：需要开启实验性功能以在静态网站生成模式中使用  NextJS 图像。
  // 请参见 https://nextjs.org/docs/messages/export-image-api 来了解不同的解决方法。
  images: {
    unoptimized: true,
  },
};

module.exports = nextConfig;
