use crate::card::{Card, Rank};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HandRank {
    HighCard,
    OnePair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
    RoyalFlush,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedHand {
    pub rank: HandRank,
    pub cards: Vec<Card>, // 5 cards that make up the combination
}

impl Ord for EvaluatedHand {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank.cmp(&other.rank)
            .then_with(|| self.cards[..].cmp(&other.cards[..]))
    }
}

impl PartialOrd for EvaluatedHand {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Evaluates the best poker hand from 7 cards
pub fn evaluate_hand(cards: &[Card]) -> EvaluatedHand {
    let mut best = EvaluatedHand {
        rank: HandRank::HighCard,
        cards: vec![],
    };
    // Iterate over all 21 possible 5-card combinations from 7 cards
    let n = cards.len();
    let mut indices = [0, 1, 2, 3, 4];
    while indices[0] <= n - 5 {
        let hand = vec![
            cards[indices[0]],
            cards[indices[1]],
            cards[indices[2]],
            cards[indices[3]],
            cards[indices[4]],
        ];
        let eval = evaluate_five(&hand);
        if eval > best {
            best = eval;
        }
        // Next combination
        let mut i = 4;
        while i > 0 && indices[i] == n - 5 + i {
            i -= 1;
        }
        indices[i] += 1;
        for j in i+1..5 {
            indices[j] = indices[j-1] + 1;
        }
        if indices[0] > n - 5 {
            break;
        }
    }
    best
}

fn evaluate_five(cards: &[Card]) -> EvaluatedHand {
    let mut sorted = cards.to_vec();
    sorted.sort_by(|a, b| b.rank.cmp(&a.rank));
    let is_flush = sorted.iter().all(|c| c.suit == sorted[0].suit);
    let straight_high = straight_high_card(&sorted);
    // Straight flush and royal flush
    if is_flush && straight_high.is_some() {
        let high = straight_high.unwrap();
        if high == Rank::Ace {
            return EvaluatedHand { rank: HandRank::RoyalFlush, cards: sorted.clone() };
        } else {
            return EvaluatedHand { rank: HandRank::StraightFlush, cards: sorted.clone() };
        }
    }
    // Four of a kind, full house, three of a kind, two pair, pair
    let mut rank_counts = HashMap::new();
    for c in &sorted {
        *rank_counts.entry(c.rank).or_insert(0) += 1;
    }
    let mut counts: Vec<(Rank, usize)> = rank_counts.iter().map(|(&r, &c)| (r, c)).collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
    if counts[0].1 == 4 {
        // Four of a kind
        let kicker = sorted.iter().find(|c| c.rank != counts[0].0).unwrap();
        let mut hand = sorted.iter().filter(|c| c.rank == counts[0].0).cloned().collect::<Vec<_>>();
        hand.push(*kicker);
        return EvaluatedHand { rank: HandRank::FourOfAKind, cards: hand };
    }
    if counts[0].1 == 3 && counts.len() > 1 && counts[1].1 >= 2 {
        // Full house
        let mut hand = sorted.iter().filter(|c| c.rank == counts[0].0).cloned().collect::<Vec<_>>();
        hand.extend(sorted.iter().filter(|c| c.rank == counts[1].0).take(2).cloned());
        return EvaluatedHand { rank: HandRank::FullHouse, cards: hand };
    }
    if is_flush {
        return EvaluatedHand { rank: HandRank::Flush, cards: sorted.clone() };
    }
    if straight_high.is_some() {
        return EvaluatedHand { rank: HandRank::Straight, cards: sorted.clone() };
    }
    if counts[0].1 == 3 {
        // Three of a kind
        let mut hand = sorted.iter().filter(|c| c.rank == counts[0].0).cloned().collect::<Vec<_>>();
        hand.extend(sorted.iter().filter(|c| c.rank != counts[0].0).take(2).cloned());
        return EvaluatedHand { rank: HandRank::ThreeOfAKind, cards: hand };
    }
    if counts[0].1 == 2 && counts.len() > 1 && counts[1].1 == 2 {
        // Two pair
        let mut hand = sorted.iter().filter(|c| c.rank == counts[0].0).cloned().collect::<Vec<_>>();
        hand.extend(sorted.iter().filter(|c| c.rank == counts[1].0).cloned());
        hand.push(sorted.iter().find(|c| c.rank != counts[0].0 && c.rank != counts[1].0).unwrap().clone());
        return EvaluatedHand { rank: HandRank::TwoPair, cards: hand };
    }
    if counts[0].1 == 2 {
        // One pair
        let mut hand = sorted.iter().filter(|c| c.rank == counts[0].0).cloned().collect::<Vec<_>>();
        hand.extend(sorted.iter().filter(|c| c.rank != counts[0].0).take(3).cloned());
        return EvaluatedHand { rank: HandRank::OnePair, cards: hand };
    }
    // High card
    EvaluatedHand { rank: HandRank::HighCard, cards: sorted.clone() }
}

fn straight_high_card(cards: &[Card]) -> Option<Rank> {
    let mut ranks: Vec<Rank> = cards.iter().map(|c| c.rank).collect();
    ranks.sort_by(|a, b| b.cmp(a));
    ranks.dedup();
    if ranks.len() < 5 {
        return None;
    }
    for i in 0..=ranks.len() - 5 {
        if ranks[i] as u8 == ranks[i + 4] as u8 + 4 {
            return Some(ranks[i]);
        }
    }
    // Wheel: A-2-3-4-5
    if ranks.contains(&Rank::Ace)
        && ranks.contains(&Rank::Five)
        && ranks.contains(&Rank::Four)
        && ranks.contains(&Rank::Three)
        && ranks.contains(&Rank::Two)
    {
        return Some(Rank::Five);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Rank, Suit};

    #[test]
    fn test_high_card() {
        let cards = vec![
            Card { rank: Rank::Ace, suit: Suit::Spades },
            Card { rank: Rank::Seven, suit: Suit::Hearts },
            Card { rank: Rank::Four, suit: Suit::Diamonds },
            Card { rank: Rank::Jack, suit: Suit::Clubs },
            Card { rank: Rank::Ten, suit: Suit::Spades },
            Card { rank: Rank::Nine, suit: Suit::Hearts },
            Card { rank: Rank::Three, suit: Suit::Diamonds },
        ];
        let hand = evaluate_hand(&cards);
        assert_eq!(hand.rank, HandRank::HighCard);
        assert_eq!(hand.cards[0].rank, Rank::Ace);
    }
}
