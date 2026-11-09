import io
import json
import unittest
import urllib.error
from unittest import mock

from scripts import aggregate_st_test_flake_history as trend

URL = "https://api.github.com/repos/owner/repo/actions/artifacts?per_page=100&page=1"


def answer(payload: object) -> io.BytesIO:
    return io.BytesIO(json.dumps(payload).encode("utf-8"))


def http_error(code: int) -> urllib.error.HTTPError:
    return urllib.error.HTTPError(URL, code, "error", None, None)


class AskingGitHubAgainTests(unittest.TestCase):
    """Specification 22: the trend asks again after a server error, up to three more times."""

    def setUp(self) -> None:
        urlopen = mock.patch.object(trend.urllib.request, "urlopen")
        sleep = mock.patch.object(trend.time, "sleep")
        self.urlopen = urlopen.start()
        self.sleep = sleep.start()
        self.addCleanup(urlopen.stop)
        self.addCleanup(sleep.stop)

    def pauses(self) -> list[float]:
        return [call.args[0] for call in self.sleep.call_args_list]

    def test_a_server_error_is_asked_again_after_a_pause_until_github_answers(self) -> None:
        # Catches one 500 from the API failing the whole nightly job, and asking again without a pause.
        self.urlopen.side_effect = [http_error(500), http_error(502), answer({"artifacts": []})]

        payload = trend.github_api_get_json(URL, "token")

        self.assertEqual(payload, {"artifacts": []})
        self.assertEqual(self.urlopen.call_count, 3)
        self.assertEqual(self.pauses(), [5, 15])

    def test_a_lasting_server_error_fails_on_the_fourth_attempt(self) -> None:
        # Catches asking forever, and a lasting outage passing as an empty trend.
        self.urlopen.side_effect = [http_error(503) for _ in range(4)]

        with self.assertRaises(urllib.error.HTTPError) as raised:
            trend.github_api_get_json(URL, "token")

        self.assertEqual(raised.exception.code, 503)
        self.assertEqual(self.urlopen.call_count, 4)
        self.assertEqual(self.pauses(), [5, 15, 45])

    def test_a_client_error_fails_at_once(self) -> None:
        # Catches waiting over a minute on a wrong address or a refused token.
        self.urlopen.side_effect = [http_error(404)]

        with self.assertRaises(urllib.error.HTTPError) as raised:
            trend.github_api_get_json(URL, "token")

        self.assertEqual(raised.exception.code, 404)
        self.assertEqual(self.urlopen.call_count, 1)
        self.assertEqual(self.pauses(), [])


if __name__ == "__main__":
    unittest.main()
