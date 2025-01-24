import { UnitLength_type } from '@kittycad/lib/dist/types/src/models'
import {
  ASK_TO_OPEN_QUERY_PARAM,
  CREATE_FILE_URL_PARAM,
  PROD_APP_URL,
} from './constants'
import { stringToBase64 } from './base64'
import { DEV, VITE_KC_API_BASE_URL } from 'env'
import toast from 'react-hot-toast'
import { err } from './trap'
export interface FileLinkParams {
  code: string
  name: string
  units: UnitLength_type
}

export async function copyFileShareLink(
  args: FileLinkParams & { token: string }
) {
  const token = args.token
  if (!token) {
    toast.error('You need to be signed in to share a file.', {
      duration: 5000,
    })
    return
  }
  const shareUrl = createCreateFileUrl(args)
  const shortlink = await createShortlink(token, shareUrl.toString())

  if (err(shortlink)) {
    toast.error(shortlink.message, {
      duration: 5000,
    })
    return
  }

  await globalThis.navigator.clipboard.writeText(shortlink.url)
  toast.success(
    'Link copied to clipboard. Anyone who clicks this link will get a copy of this file. Share carefully!',
    {
      duration: 5000,
    }
  )
}

/**
 * Creates a URL with the necessary query parameters to trigger
 * the "Import file from URL" command in the app.
 *
 * With the additional step of asking the user if they want to
 * open the URL in the desktop app.
 */
export function createCreateFileUrl({ code, name, units }: FileLinkParams) {
  // Use the dev server if we are in development mode
  let origin = DEV ? 'http://localhost:3000' : PROD_APP_URL
  const searchParams = new URLSearchParams({
    [CREATE_FILE_URL_PARAM]: String(true),
    name,
    units,
    code: stringToBase64(code),
    [ASK_TO_OPEN_QUERY_PARAM]: String(true),
  })
  const createFileUrl = new URL(`?${searchParams.toString()}`, origin)

  return createFileUrl
}

/**
 * Given a file's code, name, and units, creates shareable link to the
 * web app with a query parameter that triggers a modal to "open in desktop app".
 * That modal is defined in the `OpenInDesktopAppHandler` component.
 * TODO: update the return type to use TS library after its updated
 */
export async function createShortlink(
  token: string,
  url: string
): Promise<Error | { key: string; url: string }> {
  /**
   * We don't use our `withBaseURL` function here because
   * there is no URL shortener service in the dev API.
   */
  const response = await fetch(`${VITE_KC_API_BASE_URL}/user/shortlinks`, {
    method: 'POST',
    headers: {
      'Content-type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({
      url,
      // In future we can support org-scoped and password-protected shortlinks here
      // https://zoo.dev/docs/api/shortlinks/create-a-shortlink-for-a-user?lang=typescript
    }),
  })
  if (!response.ok) {
    const error = await response.json()
    return new Error(`Failed to create shortlink: ${error.message}`)
  } else {
    return response.json()
  }
}
