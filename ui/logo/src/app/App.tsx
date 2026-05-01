import LockitIcon from './components/LockitIcon';

export default function App() {
  return (
    <div className="min-h-screen bg-zinc-900 flex items-center justify-center p-8">
      <div className="text-center space-y-12">
        {/* Icon Display */}
        <div className="flex flex-col items-center gap-8">
          <div className="bg-black/40 backdrop-blur-sm rounded-sm p-12 shadow-2xl border border-zinc-800">
            <LockitIcon size={200} />
          </div>
          <h1 className="text-6xl font-bold text-zinc-100 tracking-tight">LOCKIT</h1>
          <p className="text-xl text-zinc-400 max-w-md font-light">
            安全管理你的 API Keys、Tokens、SSH 密钥和 Cookies
          </p>
        </div>

        {/* Different Sizes */}
        <div className="space-y-6">
          <h2 className="text-2xl font-semibold text-zinc-100">不同尺寸预览</h2>
          <div className="flex items-end justify-center gap-8">
            <div className="flex flex-col items-center gap-2">
              <div className="bg-black/40 backdrop-blur-sm rounded-sm p-4 border border-zinc-800">
                <LockitIcon size={64} />
              </div>
              <span className="text-zinc-400 text-sm">64px</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <div className="bg-black/40 backdrop-blur-sm rounded-sm p-4 border border-zinc-800">
                <LockitIcon size={96} />
              </div>
              <span className="text-zinc-400 text-sm">96px</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <div className="bg-black/40 backdrop-blur-sm rounded-sm p-4 border border-zinc-800">
                <LockitIcon size={128} />
              </div>
              <span className="text-zinc-400 text-sm">128px</span>
            </div>
          </div>
        </div>

        {/* Dark/Light Background */}
        <div className="space-y-6">
          <h2 className="text-2xl font-semibold text-zinc-100">不同背景效果</h2>
          <div className="grid grid-cols-3 gap-6 max-w-2xl mx-auto">
            <div className="flex flex-col items-center gap-3">
              <div className="bg-zinc-100 rounded-sm p-6 w-full flex justify-center border border-zinc-300">
                <LockitIcon size={80} variant="light" />
              </div>
              <span className="text-zinc-400 text-sm">浅色背景</span>
            </div>
            <div className="flex flex-col items-center gap-3">
              <div className="bg-black rounded-sm p-6 w-full flex justify-center border border-zinc-800">
                <LockitIcon size={80} variant="dark" />
              </div>
              <span className="text-zinc-400 text-sm">深色背景</span>
            </div>
            <div className="flex flex-col items-center gap-3">
              <div className="bg-zinc-800 rounded-sm p-6 w-full flex justify-center border border-zinc-700">
                <LockitIcon size={80} variant="dark" />
              </div>
              <span className="text-zinc-400 text-sm">工业风</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}