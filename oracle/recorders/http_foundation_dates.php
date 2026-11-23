<?php
// The date functions Cookie and HeaderBag call, for
// crates/php-value/tests/data/dates.json (scripts/record.sh http-foundation):
// strtotime($text, BASE) over a corpus, DateTimeImmutable::createFromFormat
// (DATE_RFC2822, $text) (HeaderBag::getDate()), and gmdate(DATE_COOKIE-like
// formats) over timestamps. Texts are base64. The default time zone is
// recorded: strtotime() reads a text without a zone in it.

error_reporting(E_ALL);

const BASE = 1700000000; // Tue, 14 Nov 2023 22:13:20 GMT

$words = [
    'invalid', 'bar', 'abcde', 'foo', 'Tue', 'tuesday', 'now', 'today', 'tomorrow', 'yesterday',
    'midnight', 'noon', 'a', 'j', 'z', 'Z', 'x', 'GMT', 'UTC', 'utc', 'EST', 'CET', 'deleted',
    'never', 'session', 'true', 'false', 'null', 'none', 'expired', 'max', '0x10', 'infinity',
    'Thu', 'Thursday', 'May', 'may', 'sept', 'Sept', 'next', 'last', 'ago', 'first', 'third',
    'week', 'day', 'monday next week', 'next monday', 'last friday', 'this week',
];
$dates = [
    'Fri, 20 May 2011 15:25:52 GMT', 'Tue, 22 Sep 2020 06:27:09 GMT', 'Wed, 16 Feb 2011 14:17:43 GMT',
    'Thu, 23 Mar 2017 09:15:12 GMT', 'Tue, 14 Nov 2023 22:13:20 GMT', 'Tue, 14-Nov-2023 22:13:20 GMT',
    'Tue, 14-Nov-23 22:13:20 GMT', 'Tuesday, 14-Nov-23 22:13:20 GMT', 'Sunday, 06-Nov-94 08:49:37 GMT',
    'Sun Nov  6 08:49:37 1994', 'Sun Nov 6 08:49:37 1994', 'Sun, 06 Nov 1994 08:49:37 GMT',
    // wrong weekday
    'Sat, 20 May 2011 15:25:52 GMT', 'Mon, 20 May 2011 15:25:52 GMT', 'Fri, 21 May 2011 15:25:52 GMT',
    // zones
    'Fri, 20 May 2011 15:25:52 UTC', 'Fri, 20 May 2011 15:25:52 utc', 'Fri, 20 May 2011 15:25:52 gmt',
    'Fri, 20 May 2011 15:25:52 Z', 'Fri, 20 May 2011 15:25:52 +0200', 'Fri, 20 May 2011 15:25:52 -0530',
    'Fri, 20 May 2011 15:25:52 +02:00', 'Fri, 20 May 2011 15:25:52 +02', 'Fri, 20 May 2011 15:25:52 GMT+2',
    'Fri, 20 May 2011 15:25:52 EST', 'Fri, 20 May 2011 15:25:52 PST', 'Fri, 20 May 2011 15:25:52 CET',
    'Fri, 20 May 2011 15:25:52', 'Fri, 20 May 2011 15:25:52 XYZ', 'Fri, 20 May 2011 15:25:52 GMT GMT',
    // no weekday, separators, case
    '20 May 2011 15:25:52 GMT', '20 May 2011', 'May 20 2011', 'May 20, 2011', '20-May-2011', '20-may-2011',
    'fri, 20 may 2011 15:25:52 gmt', 'FRI, 20 MAY 2011 15:25:52 GMT', 'Fri,20 May 2011 15:25:52 GMT',
    'Fri, 20 May 2011 15:25 GMT', 'Fri, 20 May 2011 5:25:52 GMT', 'Fri, 20 May 2011 15:25:52.5 GMT',
    'Fri, 20 May 2011  15:25:52  GMT', ' Fri, 20 May 2011 15:25:52 GMT ', "Fri, 20 May 2011 15:25:52 GMT\t",
    'Fri, 20 Mai 2011 15:25:52 GMT', 'Fri, 20 May 11 15:25:52 GMT', 'Fri, 20 May 69 15:25:52 GMT',
    'Fri, 20 May 70 15:25:52 GMT', 'Fri, 20 May 0011 15:25:52 GMT', 'Fri, 20 May 99999 15:25:52 GMT',
    'Fri, 20 September 2011 15:25:52 GMT', 'Fri, 20 Sept 2011 15:25:52 GMT',
    // out of range
    'Fri, 31 Feb 2011 15:25:52 GMT', 'Fri, 32 May 2011 15:25:52 GMT', 'Fri, 00 May 2011 15:25:52 GMT',
    'Fri, 20 May 2011 24:00:00 GMT', 'Fri, 20 May 2011 25:00:00 GMT', 'Fri, 20 May 2011 15:60:52 GMT',
    'Fri, 20 May 2011 15:25:60 GMT', 'Fri, 20 May 2011 15:25:61 GMT', 'Thu, 29 Feb 2024 00:00:00 GMT',
    'Fri, 20 May 1969 15:25:52 GMT', 'Thu, 01 Jan 1970 00:00:00 GMT', 'Thu, 01 Jan 1970 00:00:01 GMT',
    // ISO and others
    '2011-05-20', '2011-05-20 15:25:52', '2011-05-20T15:25:52', '2011-05-20T15:25:52Z', '2011-05-20T15:25:52+02:00',
    '2011-05-20 15:25', '20110520', '2011/05/20', '05/20/2011', '20.05.2011', '@1234567890', '@-1', '@0',
    // relative
    '+1 day', '-1 day', '+1 days', '+2 weeks', '+1 hour', '1 hour ago', '+1 month', '+1 year', '+30 minutes',
    '+1 sec', '+1 second', '+0 day', '+3600 seconds', '+1 week 2 days', '-1 week', 'now +1 day', '+1 day +2 hours',
    '+1 fortnight', '+1 weekday', '+1 Day', '+1DAY', '1 day', '+ 1 day',
    // numbers and junk
    '1234567890', '0', '', ' ', '-', '+', '1', '12', '123', '1234', '12345', '1.5', 'Tue,', ',', ';', '"', '20',
    'Fri, 20 May 2011 15:25:52 GMT; path=/', 'Fri, 20 May 2011 15:25:52 GMTx', '15:25:52', '15:25', 'Fri',
];
$corpus = array_values(array_unique(array_merge($words, $dates)));

$tz = date_default_timezone_get();
$strtotime = [];
foreach ($corpus as $text) {
    $r = strtotime($text, BASE);
    $strtotime[] = [base64_encode($text), false === $r ? null : $r];
}

$rfc2822 = [];
$rfcCorpus = array_merge($corpus, [
    'Tue, 4 Sep 2012 20:00:00 +0200', 'Tue', 'Tue, 04 Sep 2012 20:00:00 +0200', 'Mon, 4 Sep 2012 20:00:00 +0200',
    'Tue, 4 Sep 2012 20:00:00 GMT', 'Tue, 4 Sep 2012 20:00:00 UTC', 'Tue, 4 Sep 2012 20:00:00 Z',
    'Tue, 4 Sep 2012 20:00:00 +02:00', 'Tue, 4 Sep 2012 20:00:00 EST', 'Tue, 4 Sep 2012 20:00:00 Europe/Paris',
    'Tue, 4 Sep 2012 20:00:00', 'Tue, 4 Sep 12 20:00:00 +0200', 'Tue, 4 Sep 012 20:00:00 +0200',
    'Tue, 4 Sep 20120 20:00:00 +0200', 'Tue, 4 Sep 2012 20:00 +0200', 'Tue, 4 Sep 2012 2:0:0 +0200',
    'Tue, 4 sep 2012 20:00:00 +0200', 'tue, 4 Sep 2012 20:00:00 +0200', 'Tuesday, 4 Sep 2012 20:00:00 +0200',
    'Tue, 4 September 2012 20:00:00 +0200', 'Tue,  4 Sep 2012 20:00:00 +0200', 'Tue, 4 Sep 2012 20:00:00 +0200 ',
    'Tue, 31 Feb 2012 20:00:00 +0200', 'Tue, 4 Sep 2012 25:00:00 +0200', 'Tue, 4 Sep 2012 20:61:00 +0200',
    'Xyz, 4 Sep 2012 20:00:00 +0200', 'Tue 4 Sep 2012 20:00:00 +0200', 'Tue, 4-Sep-2012 20:00:00 +0200',
    'Tue, 4 Sep 2012 20:00:00 +2', 'Tue, 4 Sep 2012 20:00:00 +020', 'Tue, 4 Sep 2012 20:00:00 +99',
    'Tue, 4 Sep 2012 20:00:00 +2400', 'Tue, 4 Sep 2012 20:00:00 -0000', 'Tue, 4 Sep 2012 20:00:00 a',
    'Thu, 01 Jan 1970 00:00:00 +0000', 'Fri, 31 Dec 9999 23:59:59 +0000', 'Fri, 1 Jan 0000 00:00:00 +0000',
]);
foreach (array_values(array_unique($rfcCorpus)) as $text) {
    $d = \DateTimeImmutable::createFromFormat(\DATE_RFC2822, $text);
    $rfc2822[] = [base64_encode($text), false === $d ? null : [(int) $d->format('U'), (int) $d->format('u'), $d->getTimezone()->getName(), $d->format('Y-m-d H:i:s.u T P')]];
}

$gmdate = [];
foreach ([0, 1, -1, 1700000000, 1305905152, 951782400, 4102444800, -86400 * 365 * 100, PHP_INT_MAX >> 20, 2147483647, 2147483648, 253402300799, 253402300800] as $ts) {
    $gmdate[] = [$ts, gmdate('D, d M Y H:i:s T', $ts), gmdate('D, d-M-Y H:i:s T', $ts), gmdate('D, d M Y H:i:s', $ts)];
}

echo json_encode([
    'timezone' => $tz,
    'base' => BASE,
    'strtotime' => $strtotime,
    'rfc2822' => $rfc2822,
    'gmdate' => $gmdate,
], JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
