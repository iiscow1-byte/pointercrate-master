statsviewer = Stats Viewer
    .rank = Challenge list rank
    .score = Challenge list score
    .stats = Challenge list stats
    .hardest = Hardest Challenge

    .completed = Challenges completed
    .completed-main = Main list Challenges completed
    .completed-extended = Extended list Challenges completed
    .completed-legacy = Legacy list Challenges completed

    .created = Challenges created
    .published = Challenges published
    .verified = Challenges verified
    .progress = Progress on

    .stats-value = { $main } Main, { $extended } Extended, { $legacy } Legacy
    .value-none = None

statsviewer-individual = Individual
    .welcome = Click on a player's name on the left to get started!

    .option-international = International

statsviewer-nation = Nations
    .welcome = Click on a country's name on the left to get started!

    .players = Players
    .unbeaten = Unbeaten Challenges

    .created-tooltip = (Co)created by { $players } { $players ->
            [one] player
            *[other] players
        } in this country:
    .published-tooltip = Published by:
    .verified-tooltip = Verified by:
    .beaten-tooltip = Beaten by { $players } { $players ->
            [one] player
            *[other] players
        } in this country:
    .progress-tooltip = Achieved by { $players } { $players ->
            [one] player
            *[other] players
        } in this country:

demon-sorting-panel = Demon Sorting
    .info = The order in which completed Challenges should be listed

    .option-alphabetical = Alphabetical
    .option-position = Position

continent-panel = Continent
    .info = Select a continent below to focus the stats viewer to that continent. Select 'All' to reset selection.

    .option-all = All

    .option-asia = Asia
    .option-europe = Europe
    .option-australia = Australia
    .option-africa = Africa
    .option-northamerica = North America
    .option-southamerica = South America
    .option-centralamerica = Central America

toggle-subdivision-panel = Show Subdivisions
    .info = Whether the map should display political subdivisions.

    .option-toggle = Show political subdivisions

# { $countries } will be replaced with .info-countries, which will be
# turned into a tooltip listing all of the selectable countries
subdivision-panel = Political Subdivision
    .info = For the { $countries } you can select a state/province from the dropdown below to focus the stats viewer to that state/province.
    .info-countries = following countries

    .option-none = None
