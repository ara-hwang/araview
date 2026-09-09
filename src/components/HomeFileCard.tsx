import { Star, X } from "lucide-react"
import { cn } from "@/lib/utils"
import { useTranslation } from "react-i18next"

type HomeFileCardProps = {
  path: string
  src?: string
  isFavorite: boolean
  onOpen: (path: string) => void
  onRemove: (path: string) => void
  onToggleFavorite: (path: string) => void
}

export function HomeFileCard({
  path,
  src,
  isFavorite,
  onOpen,
  onRemove,
  onToggleFavorite
}: HomeFileCardProps) {
  const { t } = useTranslation()
  const name = path.split(/[\\/]/).pop() ?? path

  return (
    <div className="group border-border bg-card hover:border-primary relative overflow-hidden rounded-md border">
      <button
        type="button"
        onClick={() => onOpen(path)}
        className="flex w-full flex-col items-start gap-2 p-2 text-left"
        title={path}
      >
        <div className="bg-muted aspect-square w-full overflow-hidden rounded">
          {src ? (
            <img
              src={src}
              alt={name}
              loading="lazy"
              className="h-full w-full object-cover"
              draggable={false}
              onError={(e) => {
                ;(e.currentTarget as HTMLImageElement).style.visibility =
                  "hidden"
              }}
            />
          ) : null}
        </div>
        <span className="w-full truncate text-xs">{name}</span>
      </button>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onToggleFavorite(path)
        }}
        className={cn(
          "bg-background/80 absolute top-1 left-1 rounded p-1 transition-opacity",
          isFavorite ? "opacity-100" : "opacity-0 group-hover:opacity-100"
        )}
        title={isFavorite ? t("home.card.favRemove") : t("home.card.favAdd")}
        aria-label={
          isFavorite ? t("home.card.favRemove") : t("home.card.favAdd")
        }
        aria-pressed={isFavorite}
      >
        <Star
          className={cn(
            "size-3",
            isFavorite && "fill-yellow-400 text-yellow-400"
          )}
        />
      </button>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onRemove(path)
        }}
        className="bg-background/80 hover:bg-destructive/10 hover:text-destructive dark:hover:bg-destructive/20 absolute top-1 right-1 rounded p-1 opacity-0 transition-opacity group-hover:opacity-100"
        title={t("home.card.remove")}
        aria-label={t("home.card.remove")}
      >
        <X className="size-3" />
      </button>
    </div>
  )
}
