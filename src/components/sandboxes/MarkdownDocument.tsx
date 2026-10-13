import { isValidElement, useMemo, useState, type ComponentPropsWithoutRef, type ReactNode } from 'react'
import GithubSlugger from 'github-slugger'
import { Check, Copy } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import rehypeSlug from 'rehype-slug'

// Obsidian-like read-only markdown styling via Tailwind's arbitrary child
// selectors — no @tailwindcss/typography plugin is installed, and markdown
// only renders through MarkdownDocument, so adding one just for this wasn't
// worth it.
const MARKDOWN_CLASS =
  'flex flex-col gap-4 text-sm text-foreground/55 break-words *:leading-[1.75] ' +
  '[&_h1]:mt-6 [&_h1]:border-b [&_h1]:border-border [&_h1]:pb-2 [&_h1]:text-xl [&_h1]:font-semibold [&_h1]:text-foreground/75 [&_h1:first-child]:mt-0 ' +
  '[&_h2]:mt-5 [&_h2]:text-base [&_h2]:font-semibold [&_h2]:text-foreground/75 ' +
  '[&_h3]:mt-4 [&_h3]:text-sm [&_h3]:font-medium [&_h3]:text-foreground/75 ' +
  '[&_strong]:text-foreground/75 [&_b]:text-foreground/75 ' +
  '[&_ul]:list-disc [&_ul]:space-y-1 [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:space-y-1 [&_ol]:pl-5 ' +
  '[&_code]:rounded [&_code]:border [&_code]:border-[#d1977f21] [&_code]:bg-[color-mix(in_oklab,#d1977f5e_20%,transparent)] [&_code]:px-1 [&_code]:py-0.5 [&_code]:font-mono [&_code]:text-xs [&_code]:text-[#d1977f] ' +
  '[&_pre]:whitespace-pre-wrap [&_pre]:break-words [&_pre]:rounded-md [&_pre]:border [&_pre]:border-border [&_pre]:border-l-2 [&_pre]:border-l-primary [&_pre]:bg-muted/30 [&_pre]:p-3 ' +
  '[&_pre>code]:border-0 [&_pre>code]:bg-transparent [&_pre>code]:p-0 [&_pre>code]:text-foreground/80 ' +
  '[&_blockquote]:border-l-2 [&_blockquote]:border-primary [&_blockquote]:rounded-r-md [&_blockquote]:bg-primary/5 [&_blockquote]:py-1 [&_blockquote]:pl-3 [&_blockquote]:text-muted-foreground ' +
  '[&_table]:block [&_table]:w-full [&_table]:overflow-x-auto [&_table]:border-collapse [&_table]:text-left ' +
  '[&_th]:border [&_th]:border-border [&_th]:bg-muted [&_th]:px-3 [&_th]:py-1.5 [&_th]:font-medium [&_th]:text-foreground/75 ' +
  '[&_td]:border [&_td]:border-border [&_td]:px-3 [&_td]:py-1.5 ' +
  '[&_a]:text-primary [&_a]:underline'

function getNodeText(node: ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(getNodeText).join('')
  if (isValidElement<{ children?: ReactNode }>(node)) return getNodeText(node.props.children)
  return ''
}

// Overrides ReactMarkdown's <pre> for fenced code blocks only — inline code
// stays a plain <code> styled by MARKDOWN_CLASS.
function CodeBlock({ children }: ComponentPropsWithoutRef<'pre'>) {
  const [copied, setCopied] = useState(false)

  return (
    <div className="group relative">
      <pre>{children}</pre>
      <button
        type="button"
        title="Copy code"
        onClick={() => {
          navigator.clipboard.writeText(getNodeText(children))
          setCopied(true)
          setTimeout(() => setCopied(false), 1500)
        }}
        className="absolute top-2 right-2 rounded-md border border-border bg-background/80 p-1 opacity-0 transition-opacity group-hover:opacity-100"
      >
        {copied ? <Check className="size-3.5 text-primary" /> : <Copy className="size-3.5 text-muted-foreground" />}
      </button>
    </div>
  )
}

type Heading = { depth: number; text: string; id: string }

// Ids must match what rehypeSlug assigns to the rendered headings below, so
// this uses the same slugger package and skips fenced code blocks (where a
// "#" is just a shell comment, not a heading).
function extractHeadings(markdown: string): Heading[] {
  const slugger = new GithubSlugger()
  const headings: Heading[] = []
  let inFence = false
  for (const line of markdown.split('\n')) {
    if (/^(```|~~~)/.test(line.trim())) {
      inFence = !inFence
      continue
    }
    const match = !inFence && /^(#{1,3})\s+(.+?)\s*#*$/.exec(line)
    if (match) {
      const text = match[2]
        .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
        .replace(/[*_`]/g, '')
        .trim()
      headings.push({ depth: match[1].length, text, id: slugger.slug(text) })
    }
  }
  return headings
}

/** Read-only Markdown with a table of contents (when there's more than one heading) and copyable code blocks. */
export function MarkdownDocument({ content }: { content: string }) {
  const toc = useMemo(() => extractHeadings(content), [content])

  return (
    <>
      {toc.length > 1 && (
        <nav className="mb-4 flex flex-col gap-1 rounded-md border border-border bg-muted/20 p-3 text-xs">
          <span className="mb-1 font-medium text-muted-foreground">Contents</span>
          {toc.map((heading) => (
            <a
              key={heading.id}
              href={`#${heading.id}`}
              style={{ paddingLeft: (heading.depth - 1) * 12 }}
              className="text-primary hover:underline"
              onClick={(e) => {
                e.preventDefault()
                document.getElementById(heading.id)?.scrollIntoView({ behavior: 'smooth', block: 'start' })
              }}
            >
              {heading.text}
            </a>
          ))}
        </nav>
      )}
      <div className={MARKDOWN_CLASS}>
        <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeSlug]} components={{ pre: CodeBlock }}>
          {content}
        </ReactMarkdown>
      </div>
    </>
  )
}
