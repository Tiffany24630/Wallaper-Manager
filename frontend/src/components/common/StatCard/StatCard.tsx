interface Props {
  title: string;
  value: string | number;
  subtitle?: string;
}

export default function StatCard({
  title,
  value,
  subtitle,
}: Props) {
  return (
    <div
      className="
        rounded-xl
        border
        border-zinc-800
        bg-zinc-900
        p-5
      "
    >
      <div className="text-zinc-500 text-sm">
        {title}
      </div>

      <div className="text-3xl font-bold mt-2">
        {value}
      </div>

      {subtitle && (
        <div className="text-zinc-400 text-sm mt-2">
          {subtitle}
        </div>
      )}
    </div>
  );
}